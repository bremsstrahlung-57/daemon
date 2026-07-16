export type SpriteRegion = {
  x: number;
  y: number;
  width: number;
  height: number;
};

export type SpriteFrame = {
  duration: number;
  visible: boolean;
  region: SpriteRegion;
};

export type SpriteAnimation = {
  sheet: string;
  sheetWidth: number;
  sheetHeight: number;
  frameWidth: number;
  frameHeight: number;
  frames: readonly SpriteFrame[];
  fallbackFrame: number;
};

const frame = (
  duration: number,
  x: number,
  y: number,
  visible = true,
): SpriteFrame => ({
  duration,
  visible,
  region: { x, y, width: 64, height: 64 },
});

export const MASCOT_MANIFEST = {
  idle: {
    sheet: "/wizard_hat/wizard_hat.png",
    sheetWidth: 64,
    sheetHeight: 64,
    frameWidth: 64,
    frameHeight: 64,
    frames: [frame(1000, 0, 0)],
    fallbackFrame: 0,
  },
  blinking: {
    sheet: "/wizard_hat/wizard_hat_blinking.png",
    sheetWidth: 192,
    sheetHeight: 192,
    frameWidth: 64,
    frameHeight: 64,
    frames: [
      frame(200, 0, 0),
      frame(200, 64, 0),
      frame(200, 128, 0),
      frame(100, 0, 64),
      frame(200, 64, 64),
      frame(200, 128, 64),
      frame(100, 0, 128),
      frame(100, 64, 128),
      frame(100, 128, 128),
    ],
    fallbackFrame: 0,
  },
  listening: {
    sheet: "/wizard_hat/wizard_hat1.png",
    sheetWidth: 192,
    sheetHeight: 192,
    frameWidth: 64,
    frameHeight: 64,
    frames: [
      frame(200, 0, 0),
      frame(200, 64, 0),
      frame(200, 128, 0),
      frame(200, 0, 64),
      frame(200, 64, 64),
    ],
    fallbackFrame: 0,
  },
  speaking: {
    sheet: "/wizard_hat/wizard_hat_talking.png",
    sheetWidth: 192,
    sheetHeight: 192,
    frameWidth: 64,
    frameHeight: 64,
    frames: [
      frame(100, 0, 0),
      frame(100, 64, 0),
      frame(100, 128, 0),
      frame(100, 0, 64),
      frame(150, 64, 64),
      frame(150, 128, 64),
    ],
    fallbackFrame: 0,
  },
  sleeping: {
    sheet: "/wizard_hat/wizard_hat_sleeping.png",
    sheetWidth: 128,
    sheetHeight: 128,
    frameWidth: 64,
    frameHeight: 64,
    frames: [
      frame(500, 0, 0),
      frame(500, 64, 0),
      frame(500, 0, 64),
      frame(500, 64, 64),
    ],
    fallbackFrame: 0,
  },
  dragged: {
    sheet: "/wizard_hat/wizard_hat_dragged.png",
    sheetWidth: 128,
    sheetHeight: 128,
    frameWidth: 64,
    frameHeight: 64,
    frames: [frame(200, 0, 0), frame(200, 64, 0)],
    fallbackFrame: 0,
  },
  working: {
    sheet: "/wizard_hat/wizard_hat_back.png",
    sheetWidth: 64,
    sheetHeight: 64,
    frameWidth: 64,
    frameHeight: 64,
    frames: [frame(1000, 0, 0)],
    fallbackFrame: 0,
  },
  completed: {
    sheet: "/wizard_hat/wizard_hat_happy.png",
    sheetWidth: 64,
    sheetHeight: 64,
    frameWidth: 64,
    frameHeight: 64,
    frames: [frame(1000, 0, 0)],
    fallbackFrame: 0,
  },
  failed: {
    sheet: "/wizard_hat/wizard_hat_not_happy.png",
    sheetWidth: 64,
    sheetHeight: 64,
    frameWidth: 64,
    frameHeight: 64,
    frames: [frame(1000, 0, 0)],
    fallbackFrame: 0,
  },
} as const satisfies Record<string, SpriteAnimation>;

export type MascotAnimationName = keyof typeof MASCOT_MANIFEST;

export const MASCOT_GIFS: Partial<Record<MascotAnimationName, string>> = {
  blinking: "/wizard_hat/wizard_hat_blinking.gif",
  listening: "/wizard_hat/wizard_hat1.gif",
  speaking: "/wizard_hat/wizard_hat_talking.gif",
  sleeping: "/wizard_hat/wizard_hat_sleeping.gif",
  dragged: "/wizard_hat/wizard_hat_dragged.gif",
};

export const MASCOT_ASSETS = [
  ...Object.values(MASCOT_MANIFEST).map((animation) => animation.sheet),
  ...Object.values(MASCOT_GIFS),
];
