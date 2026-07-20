import { useEffect, useState, type FormEvent } from "react";
import {
  deleteProvider,
  deleteProviderKey,
  listProviders,
  saveProvider,
  getScreenAwareSettings,
  getScreenAwareModelStatus,
  captureScreenObservation,
  saveScreenAwareSettings,
  selectProvider,
  type Provider,
  type ScreenAwareSettings,
} from "../lib/daemon";
import { onScreenAwareStatus } from "../lib/events";

export type ToolboxSection = "settings" | "about";

type ProviderToolboxProps = {
  section: ToolboxSection;
  onClose: () => void;
};

const EMPTY_PROVIDER = {
  name: "",
  baseUrl: "https://api.openai.com/v1",
  model: "",
  apiKey: "",
};

type IntervalMode = "disabled" | "30" | "60" | "120" | "custom";

const intervalModeFor = (seconds: number | null): IntervalMode => {
  if (seconds === null) {
    return "disabled";
  }
  return ([30, 60, 120] as number[]).includes(seconds)
    ? `${seconds}` as IntervalMode
    : "custom";
};

const errorMessage = (error: unknown, fallback: string) =>
  typeof error === "string"
    ? error
    : error instanceof Error
      ? error.message
      : fallback;

function ProviderToolbox({ section, onClose }: ProviderToolboxProps) {
  const [providers, setProviders] = useState<Provider[]>([]);
  const [form, setForm] = useState(EMPTY_PROVIDER);
  const [editingId, setEditingId] = useState<string | undefined>();
  const [message, setMessage] = useState("");
  const [screenSettings, setScreenSettings] = useState<ScreenAwareSettings>({
    interval_seconds: null,
    updated_at: 0,
  });
  const [intervalMode, setIntervalMode] = useState<IntervalMode>("disabled");
  const [customInterval, setCustomInterval] = useState("");
  const [screenMessage, setScreenMessage] = useState("");
  const [isModelDownloading, setIsModelDownloading] = useState(false);

  const refresh = async () => {
    try {
      setProviders(await listProviders());
    } catch {
      setMessage("Couldn’t load AI providers.");
    }
  };

  useEffect(() => {
    void refresh();
    void getScreenAwareSettings().then((settings) => {
      setScreenSettings(settings);
      setIntervalMode(intervalModeFor(settings.interval_seconds));
      if (intervalModeFor(settings.interval_seconds) === "custom") {
        setCustomInterval(`${settings.interval_seconds}`);
      }
    }).catch(() => setScreenMessage("Couldn’t load Screen Aware settings."));
    void getScreenAwareModelStatus().then(setIsModelDownloading);
  }, []);

  useEffect(() => {
    let disposed = false;
    let unlisten: (() => void) | undefined;
    void onScreenAwareStatus((payload) => {
      if (payload.status === "model-downloading") {
        setIsModelDownloading(true);
        return;
      }
      if (payload.status === "model-ready") {
        setIsModelDownloading(false);
        return;
      }
      if (payload.status === "error") {
        setIsModelDownloading(false);
      }
      setScreenMessage(payload.message);
    }).then((cleanup) => {
      if (disposed) {
        cleanup();
      } else {
        unlisten = cleanup;
      }
    });
    return () => {
      disposed = true;
      unlisten?.();
    };
  }, []);

  const save = async (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault();
    try {
      await saveProvider({ ...form, id: editingId, apiKey: form.apiKey || undefined });
      setForm(EMPTY_PROVIDER);
      setEditingId(undefined);
      setMessage("Provider saved locally.");
      await refresh();
    } catch (error) {
      setMessage(error instanceof Error ? error.message : "Couldn’t save the provider.");
    }
  };

  const activate = async (provider: Provider) => {
    await selectProvider(provider.id);
    await refresh();
  };

  const remove = async (provider: Provider) => {
    await deleteProvider(provider.id);
    await refresh();
  };

  const clearKey = async (provider: Provider) => {
    await deleteProviderKey(provider.id);
    await refresh();
  };

  const edit = (provider: Provider) => {
    setEditingId(provider.id);
    setForm({
      name: provider.name,
      baseUrl: provider.base_url,
      model: provider.model,
      apiKey: "",
    });
  };

  const saveScreenAware = async (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault();
    const intervalSeconds = intervalMode === "disabled"
      ? null
      : intervalMode === "custom"
        ? Number(customInterval)
        : Number(intervalMode);
    if (intervalSeconds !== null && (!Number.isInteger(intervalSeconds) || intervalSeconds < 1)) {
      setScreenMessage("Enter a whole number of seconds.");
      return;
    }
    try {
      const settings = await saveScreenAwareSettings({
        ...screenSettings,
        interval_seconds: intervalSeconds,
      });
      setScreenSettings(settings);
      setIntervalMode(intervalModeFor(settings.interval_seconds));
      setScreenMessage(settings.interval_seconds === null ? "Screen Aware is disabled." : "Screen Aware settings saved.");
    } catch (error) {
      setScreenMessage(error instanceof Error ? error.message : "Couldn’t save Screen Aware settings.");
    }
  };

  const captureScreenNow = async () => {
    if (isModelDownloading) {
      return;
    }
    try {
      await captureScreenObservation();
    } catch (error) {
      setScreenMessage(errorMessage(error, "Couldn’t capture the screen."));
    }
  };

  return (
    <section className="toolbox-card" aria-label="Daemon toolbox">
      <header>
        <span>{section === "about" ? "Daemon" : "Settings"}</span>
        <button type="button" onClick={onClose} aria-label="Close toolbox">×</button>
      </header>
      {section === "about" ? (
        <>
          <p>Daemon v1.0.0 · Local companion · OpenAI-compatible chat endpoints.</p>
          <p>Setup your provider and api key in settings and you are ready to go.</p>
          <p><a href="https://moondream.ai/" target="_blank" rel="noopener">Moondream2</a></p>
        </>
      ) : (
        <>
          <p className="toolbox-description">Configure the AI provider, model, and API key here.</p>
          <div className="provider-list">
            {providers.map((provider) => (
              <div key={provider.id} className="provider-row">
                <button type="button" className="provider-select" onClick={() => void activate(provider)}>
                  {provider.is_active ? "●" : "○"} {provider.name} · {provider.model}
                </button>
                <span className="provider-actions">
                  <button type="button" onClick={() => edit(provider)}>Edit</button>
                  <button type="button" disabled={!provider.api_key_configured} onClick={() => void clearKey(provider)}>Clear key</button>
                  <button type="button" onClick={() => void remove(provider)}>Remove</button>
                </span>
              </div>
            ))}
          </div>
          <form className="provider-form" onSubmit={save}>
            <input value={form.name} onChange={(event) => setForm({ ...form, name: event.target.value })} placeholder="Provider name" />
            <input value={form.baseUrl} onChange={(event) => setForm({ ...form, baseUrl: event.target.value })} placeholder="Base URL" />
            <input value={form.model} onChange={(event) => setForm({ ...form, model: event.target.value })} placeholder="Model name" />
            <input type="password" value={form.apiKey} onChange={(event) => setForm({ ...form, apiKey: event.target.value })} placeholder="API key" />
            <button type="submit">{editingId ? "Update provider" : "Save provider"}</button>
          </form>
          <details className={`screen-aware-settings ${isModelDownloading ? "is-downloading" : ""}`}>
            <summary>Screen Aware(Beta)</summary>
            <p>Screenshot is not saved and its processed locally. Processing may take time depending on your device.</p>
            {isModelDownloading && <p className="screen-aware-download-status">Downloading local model…</p>}
            <form className="screen-aware-form" onSubmit={saveScreenAware}>
              <label>
                Screenshot interval
                <select disabled={isModelDownloading} value={intervalMode} onChange={(event) => setIntervalMode(event.target.value as IntervalMode)}>
                  <option value="30">30 seconds</option>
                  <option value="60">60 seconds</option>
                  <option value="120">120 seconds</option>
                  <option value="disabled">Disabled (No screenshots)</option>
                  <option value="custom">Custom interval</option>
                </select>
              </label>
              {intervalMode === "custom" && (
                <label>
                  Custom seconds
                  <input
                    disabled={isModelDownloading}
                    min="1"
                    inputMode="numeric"
                    type="number"
                    value={customInterval}
                    onChange={(event) => setCustomInterval(event.target.value)}
                  />
                </label>
              )}
              <div className="screen-aware-actions">
                <button disabled={isModelDownloading} type="submit">Save Screen Aware</button>
                <button disabled={isModelDownloading} type="button" onClick={() => void captureScreenNow()}>
                  Capture now
                </button>
              </div>
            </form>
            {screenMessage && <p className="toolbox-message">{screenMessage}</p>}
          </details>
        </>
      )}
      {message && <p className="toolbox-message">{message}</p>}
    </section>
  );
}

export default ProviderToolbox;
