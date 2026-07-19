import { useEffect, useMemo, useState } from "react";
import {
  MASCOT_MANIFEST,
  type MascotAnimationName,
  type SpriteAnimation,
} from "./manifest";

export type MascotState =
  | "idle"
  | "listening"
  | "thinking"
  | "speaking"
  | "sleeping"
  | "dragged"
  | "working"
  | "completed"
  | "failed"
  | "happy"
  | "not_happy"
  | "startup";

type MascotFrame = {
  animation: MascotAnimationName;
  animationData: SpriteAnimation;
  frameIndex: number;
  reducedMotion: boolean;
};

const IDLE_BLINK_DELAY = 6500;

const stateAnimation: Record<MascotState, MascotAnimationName> = {
  idle: "idle",
  listening: "listening",
  thinking: "thinking",
  speaking: "speaking",
  sleeping: "sleeping",
  dragged: "dragged",
  working: "working",
  completed: "completed",
  failed: "failed",
  happy: "completed",
  not_happy: "failed",
  startup: "working",
};

const prefersReducedMotion = () =>
  window.matchMedia("(prefers-reduced-motion: reduce)").matches;

const animationFrame = (
  animation: MascotAnimationName,
  frameIndex: number,
  reducedMotion: boolean,
): MascotFrame => ({
  animation,
  animationData: MASCOT_MANIFEST[animation],
  frameIndex: Math.min(
    frameIndex,
    MASCOT_MANIFEST[animation].frames.length - 1,
  ),
  reducedMotion,
});

export function useMascotFrame(
  state: MascotState,
  isVisible: boolean,
): MascotFrame {
  const [reducedMotion, setReducedMotion] = useState(prefersReducedMotion);
  const [frameIndex, setFrameIndex] = useState(0);
  const [blink, setBlink] = useState(false);

  useEffect(() => {
    const mediaQuery = window.matchMedia("(prefers-reduced-motion: reduce)");
    const update = () => setReducedMotion(mediaQuery.matches);
    mediaQuery.addEventListener("change", update);
    return () => mediaQuery.removeEventListener("change", update);
  }, []);

  useEffect(() => {
    setFrameIndex(0);
    setBlink(false);
  }, [state]);

  useEffect(() => {
    if (!isVisible || reducedMotion || state !== "idle") {
      return;
    }

    const timer = window.setTimeout(() => setBlink(true), IDLE_BLINK_DELAY);
    return () => window.clearTimeout(timer);
  }, [isVisible, reducedMotion, state, blink]);

  const animation = blink ? "blinking" : stateAnimation[state];
  const animationData = MASCOT_MANIFEST[animation];

  useEffect(() => {
    if (!isVisible || reducedMotion || animationData.frames.length < 2) {
      return;
    }

    const currentFrame = animationData.frames[
      Math.min(frameIndex, animationData.frames.length - 1)
    ];
    const timer = window.setTimeout(() => {
      const nextFrame = frameIndex + 1;
      if (blink && nextFrame >= animationData.frames.length) {
        setBlink(false);
        setFrameIndex(0);
        return;
      }
      setFrameIndex(nextFrame % animationData.frames.length);
    }, currentFrame.duration);

    return () => window.clearTimeout(timer);
  }, [animationData, blink, frameIndex, isVisible, reducedMotion]);

  return useMemo(
    () =>
      animationFrame(
        animation,
        reducedMotion ? animationData.fallbackFrame : frameIndex,
        reducedMotion,
      ),
    [animation, animationData, frameIndex, reducedMotion],
  );
}
