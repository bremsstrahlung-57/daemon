//! Local, memory-only screen capture and Moondream2 inference.

use crate::{
    events::{SCREEN_AWARE_STATUS, SCREEN_OBSERVATION_CREATED},
    state::AppState,
    storage::ScreenObservationRecord,
};
use half::f16;
use image::{imageops, imageops::FilterType, RgbaImage};
use ort::{session::Session, value::Tensor};
use serde::Serialize;
use std::{
    fs::{self, File},
    io::copy,
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        Arc, Mutex,
    },
    time::Duration,
};
use tar::Archive;
use tauri::{AppHandle, Emitter, Manager};
use tokenizers::Tokenizer;
use xcap::Monitor;

const MODEL_ASSETS: &[&str] = &[
    "onnx/in/config.json",
    "onnx/in/initial_kv_caches.npy",
    "onnx/in/tokenizer.json",
    "onnx/slim/text_decoder_0.onnx",
    "onnx/slim/text_encoder.onnx",
    "onnx/slim/vision_encoder.onnx",
    "onnx/slim/vision_projection.onnx",
];
const IMAGE_SIZE: usize = 378;
const IMAGE_TOKENS: usize = 729;
const IMAGE_GRID_SIZE: usize = 27;
const SMALL_IMAGE_MAX_SIZE: u32 = 529;
const VISION_HIDDEN_SIZE: usize = 720;
const TEXT_HIDDEN_SIZE: usize = 1024;
const VOCAB_SIZE: usize = 51_200;
const EOS_TOKEN: usize = 50_256;
const CAPTION_PROMPT: [i64; 5] = [198, 198, 24_334, 1_159, 25];
const MAX_GENERATED_TOKENS: usize = 128;
const MAX_DESCRIPTION_CHARS: usize = 600;
const MAX_INTERVAL_SECONDS: i64 = 86_400;

#[derive(Clone)]
pub struct ScreenAwareService {
    archive_path: PathBuf,
    cache_dir: PathBuf,
    runtime: Arc<Mutex<Option<MoondreamRuntime>>>,
    capture_in_progress: Arc<AtomicBool>,
    monitoring_active: Arc<AtomicBool>,
    monitor_generation: Arc<AtomicU64>,
}

struct MoondreamRuntime {
    vision_encoder: Session,
    vision_projection: Session,
    text_encoder: Session,
    text_decoder: Session,
    tokenizer: Tokenizer,
    initial_cache: Vec<f16>,
}

struct DecoderOutput {
    next_token: usize,
    cache: Vec<f16>,
    cache_shape: Vec<usize>,
}

#[derive(Clone, Serialize)]
pub struct ScreenAwareStatusPayload {
    pub status: String,
    pub message: String,
}

impl ScreenAwareService {
    pub fn new(archive_path: PathBuf, cache_dir: PathBuf) -> Self {
        Self {
            archive_path,
            cache_dir,
            runtime: Arc::new(Mutex::new(None)),
            capture_in_progress: Arc::new(AtomicBool::new(false)),
            monitoring_active: Arc::new(AtomicBool::new(true)),
            monitor_generation: Arc::new(AtomicU64::new(0)),
        }
    }

    pub async fn capture_description(&self) -> Result<String, String> {
        if self.capture_in_progress.swap(true, Ordering::AcqRel) {
            return Err("Screen capture is already running".to_string());
        }

        let archive_path = self.archive_path.clone();
        let cache_dir = self.cache_dir.clone();
        let runtime = Arc::clone(&self.runtime);
        let result = tokio::task::spawn_blocking(move || {
            let image = capture_primary_monitor()?;
            let mut runtime = runtime
                .lock()
                .map_err(|_| "Local screen model is unavailable".to_string())?;
            let model = match runtime.as_mut() {
                Some(model) => model,
                None => {
                    *runtime = Some(MoondreamRuntime::load(&archive_path, &cache_dir)?);
                    runtime
                        .as_mut()
                        .ok_or_else(|| "Local screen model is unavailable".to_string())?
                }
            };
            model.describe(&image)
        })
        .await
        .map_err(|_| "Local screen inference did not complete".to_string());

        self.capture_in_progress.store(false, Ordering::Release);
        result?
    }

    pub fn is_capturing(&self) -> bool {
        self.capture_in_progress.load(Ordering::Acquire)
    }

    pub fn set_monitoring_active(&self, active: bool) {
        self.monitoring_active.store(active, Ordering::Release);
    }

    pub fn is_monitoring_active(&self) -> bool {
        self.monitoring_active.load(Ordering::Acquire)
    }

    pub fn restart_monitor(&self, app: AppHandle, interval_seconds: Option<i64>) {
        let generation = self.monitor_generation.fetch_add(1, Ordering::AcqRel) + 1;
        let Some(interval_seconds) = interval_seconds else {
            return;
        };

        tauri::async_runtime::spawn(async move {
            loop {
                tokio::time::sleep(Duration::from_secs(interval_seconds as u64)).await;
                let state = app.state::<AppState>();
                if state
                    .screen_aware
                    .monitor_generation
                    .load(Ordering::Acquire)
                    != generation
                {
                    return;
                }
                if !state.screen_aware.is_monitoring_active() || state.screen_aware.is_capturing() {
                    continue;
                }
                if let Ok(observation) = capture_and_store(&app, &state, "automatic").await {
                    if state.screen_aware.is_monitoring_active() {
                        let _ = crate::openai::turns::respond_to_screen_observation(
                            &app,
                            &state,
                            &observation,
                        )
                        .await;
                    }
                }
            }
        });
    }
}

impl MoondreamRuntime {
    fn load(archive_path: &Path, cache_dir: &Path) -> Result<Self, String> {
        extract_model_assets(archive_path, cache_dir)?;
        let slim = cache_dir.join("onnx/slim");
        let session = |name: &str| {
            Session::builder()
                .map_err(|error| error.to_string())?
                .with_intra_threads(1)
                .map_err(|error| error.to_string())?
                .commit_from_file(slim.join(name))
                .map_err(|error| error.to_string())
        };

        Ok(Self {
            vision_encoder: session("vision_encoder.onnx")?,
            vision_projection: session("vision_projection.onnx")?,
            text_encoder: session("text_encoder.onnx")?,
            text_decoder: session("text_decoder_0.onnx")?,
            tokenizer: Tokenizer::from_file(cache_dir.join("onnx/in/tokenizer.json"))
                .map_err(|error| error.to_string())?,
            initial_cache: load_initial_cache(&cache_dir.join("onnx/in/initial_kv_caches.npy"))?,
        })
    }

    fn describe(&self, image: &RgbaImage) -> Result<String, String> {
        let image_embeds = self.encode_image(image)?;
        // fixed: the bundled cache already contains BOS, so encoding it again corrupts every caption.
        let first = self.decode(
            image_embeds,
            IMAGE_TOKENS,
            self.initial_cache.clone(),
            vec![24, 2, 1, 16, 1, 64],
        )?;
        let prompt_embeds = self.encode_tokens(&CAPTION_PROMPT)?;
        let mut decoded = self.decode(
            prompt_embeds,
            CAPTION_PROMPT.len(),
            first.cache,
            first.cache_shape,
        )?;
        let mut token_ids = Vec::new();

        for _ in 0..MAX_GENERATED_TOKENS {
            if decoded.next_token == EOS_TOKEN {
                break;
            }
            token_ids.push(decoded.next_token as u32);
            let token_embeds = self.encode_tokens(&[decoded.next_token as i64])?;
            decoded = self.decode(token_embeds, 1, decoded.cache, decoded.cache_shape)?;
        }

        let description = self
            .tokenizer
            .decode(&token_ids, true)
            .map_err(|error| error.to_string())?;
        let description = description.trim();
        if description.is_empty() {
            return Err("Local screen model returned no description".to_string());
        }
        Ok(description.chars().take(MAX_DESCRIPTION_CHARS).collect())
    }

    fn encode_image(&self, image: &RgbaImage) -> Result<Vec<f16>, String> {
        let (crops, tiling) = prepare_crops(image)?;
        let mut pixels = Vec::with_capacity(crops.len() * 3 * IMAGE_SIZE * IMAGE_SIZE);
        for crop in &crops {
            pixels.extend(normalized_image(crop));
        }
        let inputs = ort::inputs! {
                "input" => Tensor::from_array(([crops.len(), 3, IMAGE_SIZE, IMAGE_SIZE], pixels.into_boxed_slice()))?,
            }
            .map_err(|error| error.to_string())?;
        let output = self
            .vision_encoder
            .run(inputs)
            .map_err(|error| error.to_string())?;
        let encoded = output["output"]
            .try_extract_tensor::<f16>()
            .map_err(|error| error.to_string())?;
        let encoded = encoded
            .as_slice()
            .ok_or_else(|| "Local vision output was not contiguous".to_string())?;
        if encoded.len() != crops.len() * IMAGE_TOKENS * VISION_HIDDEN_SIZE {
            return Err("Local vision output had an unexpected shape".to_string());
        }
        let global = &encoded[..IMAGE_TOKENS * VISION_HIDDEN_SIZE];
        let local = if tiling == (1, 1) {
            global.to_vec()
        } else {
            let local =
                stitch_local_features(&encoded[IMAGE_TOKENS * VISION_HIDDEN_SIZE..], tiling)?;
            adaptive_average_pool(&local, tiling)?
        };
        let mut projection_input = Vec::with_capacity(IMAGE_TOKENS * VISION_HIDDEN_SIZE * 2);
        for token in 0..IMAGE_TOKENS {
            let global_start = token * VISION_HIDDEN_SIZE;
            projection_input
                .extend_from_slice(&encoded[global_start..global_start + VISION_HIDDEN_SIZE]);
            projection_input
                .extend_from_slice(&local[global_start..global_start + VISION_HIDDEN_SIZE]);
        }
        let inputs = ort::inputs! {
                "input" => Tensor::from_array(([1usize, IMAGE_TOKENS, VISION_HIDDEN_SIZE * 2], projection_input.into_boxed_slice()))?,
            }
            .map_err(|error| error.to_string())?;
        let output = self
            .vision_projection
            .run(inputs)
            .map_err(|error| error.to_string())?;
        Ok(output["output"]
            .try_extract_tensor::<f16>()
            .map_err(|error| error.to_string())?
            .iter()
            .copied()
            .collect())
    }

    fn encode_tokens(&self, token_ids: &[i64]) -> Result<Vec<f16>, String> {
        let inputs = ort::inputs! {
                "input_ids" => Tensor::from_array(([1usize, token_ids.len()], token_ids.to_vec().into_boxed_slice()))?,
            }
            .map_err(|error| error.to_string())?;
        let output = self
            .text_encoder
            .run(inputs)
            .map_err(|error| error.to_string())?;
        Ok(output["inputs_embeds"]
            .try_extract_tensor::<f16>()
            .map_err(|error| error.to_string())?
            .iter()
            .copied()
            .collect())
    }

    fn decode(
        &self,
        embeds: Vec<f16>,
        sequence_len: usize,
        cache: Vec<f16>,
        cache_shape: Vec<usize>,
    ) -> Result<DecoderOutput, String> {
        let inputs = ort::inputs! {
                "inputs_embeds" => Tensor::from_array(([1usize, sequence_len, TEXT_HIDDEN_SIZE], embeds.into_boxed_slice()))?,
                "kv_cache" => Tensor::from_array((cache_shape.clone(), cache.clone().into_boxed_slice()))?,
            }
            .map_err(|error| error.to_string())?;
        let output = self
            .text_decoder
            .run(inputs)
            .map_err(|error| error.to_string())?;
        let logits = output["output"]
            .try_extract_tensor::<f16>()
            .map_err(|error| error.to_string())?;
        let logits = logits
            .as_slice()
            .ok_or_else(|| "Local text output was not contiguous".to_string())?;
        let last_logits = logits
            .get(logits.len().saturating_sub(VOCAB_SIZE)..)
            .ok_or_else(|| "Local text output was incomplete".to_string())?;
        let cache_update = output["new_kv_cache"]
            .try_extract_tensor::<f16>()
            .map_err(|error| error.to_string())?;
        let update_shape = cache_update.shape().to_vec();
        let cache = append_cache(
            &cache,
            &cache_shape,
            cache_update
                .as_slice()
                .ok_or_else(|| "Local text cache was not contiguous".to_string())?,
            &update_shape,
        )?;
        let mut cache_shape = cache_shape;
        let sequence_axis = cache_shape.len() - 2;
        cache_shape[sequence_axis] += update_shape[sequence_axis];

        Ok(DecoderOutput {
            next_token: argmax(last_logits),
            cache,
            cache_shape,
        })
    }
}

pub async fn capture_and_store(
    app: &AppHandle,
    state: &AppState,
    source: &str,
) -> Result<ScreenObservationRecord, String> {
    if source == "automatic" && !state.screen_aware.is_monitoring_active() {
        return Err("Screen Aware is paused while Daemon is dismissed".to_string());
    }
    emit_status(app, "capturing", "Capturing screen locally…");
    let result: Result<ScreenObservationRecord, String> = async {
        let description = state.screen_aware.capture_description().await?;
        if !is_usable_description(&description) {
            return Err("Local screen model returned an unusable description".to_string());
        }
        if source == "automatic" && !state.screen_aware.is_monitoring_active() {
            return Err("Screen Aware is paused while Daemon is dismissed".to_string());
        }
        let observation = state
            .storage
            .lock()
            .map_err(|_| "Local storage is unavailable".to_string())?
            .insert_screen_observation(&description, source)
            .map_err(|_| "Unable to save the screen description".to_string())?;
        let _ = app.emit(SCREEN_OBSERVATION_CREATED, &observation);
        Ok(observation)
    }
    .await;
    match &result {
        Ok(_) => emit_status(app, "ready", "Screen description saved."),
        Err(error) => emit_status(app, "error", error),
    }
    result
}

fn emit_status(app: &AppHandle, status: &str, message: &str) {
    let _ = app.emit(
        SCREEN_AWARE_STATUS,
        ScreenAwareStatusPayload {
            status: status.to_string(),
            message: message.to_string(),
        },
    );
}

pub fn validate_settings(interval_seconds: Option<i64>) -> Result<(), String> {
    if let Some(interval_seconds) = interval_seconds {
        if !(1..=MAX_INTERVAL_SECONDS).contains(&interval_seconds) {
            return Err("Screenshot intervals must be between 1 and 86400 seconds".to_string());
        }
    }
    Ok(())
}

fn capture_primary_monitor() -> Result<RgbaImage, String> {
    let monitors = Monitor::all().map_err(|error| error.to_string())?;
    let monitor = monitors
        .iter()
        .find(|monitor| monitor.is_primary().unwrap_or(false))
        .or_else(|| monitors.first())
        .ok_or_else(|| "No display is available to capture".to_string())?;
    monitor.capture_image().map_err(|error| error.to_string())
}

fn normalized_image(image: &RgbaImage) -> Vec<f16> {
    let image = imageops::resize(
        image,
        IMAGE_SIZE as u32,
        IMAGE_SIZE as u32,
        FilterType::Lanczos3,
    );
    let mut values = Vec::with_capacity(3 * IMAGE_SIZE * IMAGE_SIZE);
    for channel in 0..3 {
        for pixel in image.pixels() {
            values.push(f16::from_f32((pixel[channel] as f32 / 255.0 - 0.5) / 0.5));
        }
    }
    values
}

fn prepare_crops(image: &RgbaImage) -> Result<(Vec<RgbaImage>, (usize, usize)), String> {
    if image.width() == 0 || image.height() == 0 {
        return Err("Captured screen image is empty".to_string());
    }

    let tiling = if image.width().max(image.height()) <= SMALL_IMAGE_MAX_SIZE {
        (1, 1)
    } else {
        let aspect_ratio = image.width() as f64 / image.height() as f64;
        [(1, 2), (2, 1), (2, 2)]
            .into_iter()
            .min_by(|left, right| {
                let left_distance = (left.1 as f64 / left.0 as f64 - aspect_ratio).abs();
                let right_distance = (right.1 as f64 / right.0 as f64 - aspect_ratio).abs();
                left_distance.total_cmp(&right_distance)
            })
            .unwrap_or((1, 1))
    };

    let mut crops = Vec::with_capacity(tiling.0 * tiling.1 + 1);
    crops.push(imageops::resize(
        image,
        IMAGE_SIZE as u32,
        IMAGE_SIZE as u32,
        FilterType::CatmullRom,
    ));
    if tiling == (1, 1) {
        return Ok((crops, tiling));
    }

    let crop_width = image.width() / tiling.1 as u32;
    let crop_height = image.height() / tiling.0 as u32;
    for row in 0..tiling.0 {
        for column in 0..tiling.1 {
            let crop = imageops::crop_imm(
                image,
                column as u32 * crop_width,
                row as u32 * crop_height,
                crop_width,
                crop_height,
            )
            .to_image();
            crops.push(imageops::resize(
                &crop,
                IMAGE_SIZE as u32,
                IMAGE_SIZE as u32,
                FilterType::CatmullRom,
            ));
        }
    }
    Ok((crops, tiling))
}

fn stitch_local_features(local_crops: &[f16], tiling: (usize, usize)) -> Result<Vec<f16>, String> {
    let crop_features = IMAGE_TOKENS * VISION_HIDDEN_SIZE;
    if local_crops.len() != tiling.0 * tiling.1 * crop_features {
        return Err("Local vision crops had an unexpected shape".to_string());
    }

    let output_width = IMAGE_GRID_SIZE * tiling.1;
    let mut stitched = vec![f16::ZERO; IMAGE_TOKENS * tiling.0 * tiling.1 * VISION_HIDDEN_SIZE];
    for crop_index in 0..tiling.0 * tiling.1 {
        let crop_row = crop_index / tiling.1;
        let crop_column = crop_index % tiling.1;
        for row in 0..IMAGE_GRID_SIZE {
            let source = (crop_index * IMAGE_TOKENS + row * IMAGE_GRID_SIZE) * VISION_HIDDEN_SIZE;
            let destination = ((crop_row * IMAGE_GRID_SIZE + row) * output_width
                + crop_column * IMAGE_GRID_SIZE)
                * VISION_HIDDEN_SIZE;
            let row_len = IMAGE_GRID_SIZE * VISION_HIDDEN_SIZE;
            stitched[destination..destination + row_len]
                .copy_from_slice(&local_crops[source..source + row_len]);
        }
    }
    Ok(stitched)
}

fn adaptive_average_pool(local: &[f16], tiling: (usize, usize)) -> Result<Vec<f16>, String> {
    let input_height = IMAGE_GRID_SIZE * tiling.0;
    let input_width = IMAGE_GRID_SIZE * tiling.1;
    if local.len() != input_height * input_width * VISION_HIDDEN_SIZE {
        return Err("Reconstructed local vision features had an unexpected shape".to_string());
    }
    let mut pooled = vec![f16::ZERO; IMAGE_TOKENS * VISION_HIDDEN_SIZE];
    let stride_height = input_height / IMAGE_GRID_SIZE;
    let stride_width = input_width / IMAGE_GRID_SIZE;
    let kernel_height = input_height - (IMAGE_GRID_SIZE - 1) * stride_height;
    let kernel_width = input_width - (IMAGE_GRID_SIZE - 1) * stride_width;
    for output_y in 0..IMAGE_GRID_SIZE {
        let start_y = output_y * stride_height;
        let end_y = start_y + kernel_height;
        for output_x in 0..IMAGE_GRID_SIZE {
            let start_x = output_x * stride_width;
            let end_x = start_x + kernel_width;
            let count = ((end_y - start_y) * (end_x - start_x)) as f32;
            let destination = (output_y * IMAGE_GRID_SIZE + output_x) * VISION_HIDDEN_SIZE;
            for channel in 0..VISION_HIDDEN_SIZE {
                let mut sum = 0.0;
                for input_y in start_y..end_y {
                    for input_x in start_x..end_x {
                        sum += local
                            [(input_y * input_width + input_x) * VISION_HIDDEN_SIZE + channel]
                            .to_f32();
                    }
                }
                pooled[destination + channel] = f16::from_f32(sum / count);
            }
        }
    }
    Ok(pooled)
}

fn is_usable_description(description: &str) -> bool {
    description.split_whitespace().count() >= 2
}

fn append_cache(
    cache: &[f16],
    cache_shape: &[usize],
    update: &[f16],
    update_shape: &[usize],
) -> Result<Vec<f16>, String> {
    if cache_shape.len() < 2
        || cache_shape.len() != update_shape.len()
        || cache_shape[..cache_shape.len() - 2] != update_shape[..update_shape.len() - 2]
        || cache_shape.last() != update_shape.last()
    {
        return Err("Local text cache had an unexpected shape".to_string());
    }

    let sequence_axis = cache_shape.len() - 2;
    let prefix_count = cache_shape[..sequence_axis].iter().product::<usize>();
    let width = cache_shape[sequence_axis + 1];
    let cache_chunk = cache_shape[sequence_axis] * width;
    let update_chunk = update_shape[sequence_axis] * width;
    if cache.len() != prefix_count * cache_chunk || update.len() != prefix_count * update_chunk {
        return Err("Local text cache was incomplete".to_string());
    }

    let mut combined = Vec::with_capacity(cache.len() + update.len());
    for prefix in 0..prefix_count {
        combined.extend_from_slice(&cache[prefix * cache_chunk..(prefix + 1) * cache_chunk]);
        combined.extend_from_slice(&update[prefix * update_chunk..(prefix + 1) * update_chunk]);
    }
    Ok(combined)
}

fn argmax(values: &[f16]) -> usize {
    values
        .iter()
        .enumerate()
        .max_by(|(_, left), (_, right)| left.to_f32().total_cmp(&right.to_f32()))
        .map(|(index, _)| index)
        .unwrap_or_default()
}

fn extract_model_assets(archive_path: &Path, cache_dir: &Path) -> Result<(), String> {
    if MODEL_ASSETS
        .iter()
        .all(|asset| cache_dir.join(asset).is_file())
    {
        return Ok(());
    }
    if !archive_path.is_file() {
        return Err("The bundled local Moondream2 model is missing".to_string());
    }

    fs::create_dir_all(cache_dir).map_err(|error| error.to_string())?;
    let mut archive = Archive::new(File::open(archive_path).map_err(|error| error.to_string())?);
    for entry in archive.entries().map_err(|error| error.to_string())? {
        let mut entry = entry.map_err(|error| error.to_string())?;
        let path = entry
            .path()
            .map_err(|error| error.to_string())?
            .to_string_lossy()
            .replace('\\', "/");
        if !MODEL_ASSETS.contains(&path.as_str()) {
            continue;
        }
        let output = cache_dir.join(&path);
        if let Some(parent) = output.parent() {
            fs::create_dir_all(parent).map_err(|error| error.to_string())?;
        }
        // The cache contains model assets only; screenshot pixels never cross this boundary.
        copy(
            &mut entry,
            &mut File::create(output).map_err(|error| error.to_string())?,
        )
        .map_err(|error| error.to_string())?;
    }
    if MODEL_ASSETS
        .iter()
        .all(|asset| cache_dir.join(asset).is_file())
    {
        Ok(())
    } else {
        Err("The bundled local Moondream2 model is incomplete".to_string())
    }
}

fn load_initial_cache(path: &Path) -> Result<Vec<f16>, String> {
    let data = fs::read(path).map_err(|error| error.to_string())?;
    if data.len() < 10 || &data[..6] != b"\x93NUMPY" {
        return Err("The local Moondream2 cache is invalid".to_string());
    }
    let header_len = u16::from_le_bytes([data[8], data[9]]) as usize;
    let offset = 10 + header_len;
    if offset > data.len() || (data.len() - offset) % 2 != 0 {
        return Err("The local Moondream2 cache is invalid".to_string());
    }
    Ok(data[offset..]
        .chunks_exact(2)
        .map(|bytes| f16::from_bits(u16::from_le_bytes([bytes[0], bytes[1]])))
        .collect())
}

#[cfg(test)]
mod tests {
    use super::{
        append_cache, prepare_crops, validate_settings, MoondreamRuntime, ScreenAwareService,
    };
    use half::f16;
    use image::RgbaImage;
    use std::path::PathBuf;

    #[test]
    fn screen_aware_settings_reject_invalid_values() {
        assert!(validate_settings(Some(10)).is_ok());
        assert!(validate_settings(None).is_ok());
        assert!(validate_settings(Some(0)).is_err());
    }

    #[test]
    fn dismissed_daemon_pauses_automatic_monitoring() {
        let service = ScreenAwareService::new(PathBuf::new(), PathBuf::new());
        assert!(service.is_monitoring_active());
        service.set_monitoring_active(false);
        assert!(!service.is_monitoring_active());
        service.set_monitoring_active(true);
        assert!(service.is_monitoring_active());
    }

    #[test]
    fn decoder_cache_appends_updates_per_attention_head() {
        let cache = [1.0, 2.0, 3.0, 4.0].map(f16::from_f32).to_vec();
        let update = [5.0, 6.0, 7.0, 8.0].map(f16::from_f32).to_vec();
        let combined = append_cache(&cache, &[2, 1, 2], &update, &[2, 1, 2])
            .expect("cache update should append");
        assert_eq!(
            combined
                .iter()
                .map(|value| value.to_f32())
                .collect::<Vec<_>>(),
            vec![1.0, 2.0, 5.0, 6.0, 3.0, 4.0, 7.0, 8.0]
        );
    }

    #[test]
    fn screen_crops_keep_global_and_local_context() {
        let image = RgbaImage::new(1920, 1080);
        let (crops, tiling) = prepare_crops(&image).expect("screen crops should prepare");
        assert_eq!(tiling, (1, 2));
        assert_eq!(crops.len(), 3);
        assert!(crops.iter().all(|crop| crop.dimensions() == (378, 378)));
    }

    #[test]
    #[ignore = "requires the bundled 4-bit Moondream2 model"]
    fn bundled_model_generates_a_local_description() {
        let project = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .expect("project directory should exist")
            .to_path_buf();
        let runtime = MoondreamRuntime::load(
            &project.join("model/moondream-0_5b-int4.bin"),
            &project.join("src-tauri/target/moondream-screen-aware-test"),
        )
        .expect("bundled model should load");
        let description = runtime
            .describe(&RgbaImage::new(64, 64))
            .expect("bundled model should describe an image");
        assert!(!description.is_empty());
    }
}
