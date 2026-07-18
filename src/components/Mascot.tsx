import { useEffect } from "react";
import {
  MASCOT_ASSETS,
  MASCOT_GIFS,
  type MascotAnimationName,
} from "../mascot/manifest";
import { useMascotFrame, type MascotState } from "../mascot/state";

type MascotProps = {
  state: MascotState;
  isVisible: boolean;
};

const LABELS: Record<MascotState, string> = {
  idle: "Daemon is idle",
  listening: "Daemon is listening",
  thinking: "Daemon is thinking",
  speaking: "Daemon is speaking",
  sleeping: "Daemon is sleeping",
  dragged: "Daemon is being dragged",
  working: "Daemon is working",
  completed: "Daemon completed the task",
  failed: "Daemon could not complete the task",
  happy: "Daemon is happy",
  not_happy: "Daemon is not happy",
};

function Mascot({ state, isVisible }: MascotProps) {
  const frame = useMascotFrame(state, isVisible);

  useEffect(() => {
    MASCOT_ASSETS.forEach((source) => {
      const image = new Image();
      image.src = source;
    });
  }, []);

  const spriteFrame = frame.animationData.frames[frame.frameIndex]
    ?? frame.animationData.frames[frame.animationData.fallbackFrame];
  const { region } = spriteFrame;
  const scale = 80 / frame.animationData.frameWidth;
  const animatedSource = !frame.reducedMotion && MASCOT_GIFS[frame.animation];

  if (animatedSource) {
    return (
      <img
        alt={LABELS[state]}
        className="daemon-img"
        data-animation={frame.animation as MascotAnimationName}
        height={80}
        src={animatedSource}
        width={80}
      />
    );
  }

  return (
    <span
      aria-label={LABELS[state]}
      className="daemon-img"
      role="img"
      style={{
        backgroundImage: `url(${frame.animationData.sheet})`,
        backgroundPosition: `${-region.x * scale}px ${-region.y * scale}px`,
        backgroundSize: `${frame.animationData.sheetWidth * scale}px ${frame.animationData.sheetHeight * scale}px`,
        height: `${frame.animationData.frameHeight * scale}px`,
        width: `${frame.animationData.frameWidth * scale}px`,
      }}
      data-animation={frame.animation as MascotAnimationName}
    />
  );
}

export default Mascot;
