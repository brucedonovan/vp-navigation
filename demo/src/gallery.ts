import type { NodeContent } from "../../packages/dom/src/index";

export interface Stop {
  id: string;
  label: string;
  kicker: string;
  title: string;
  body: string;
  tone?: "annex";
}

export const mainStops: Stop[] = [
  {
    id: "threshold",
    label: "Threshold",
    kicker: "01  ·  Threshold",
    title: "Stop scrolling.",
    body: "This page does not run downward. It runs away from you, toward one point of light. What you can read is whatever is standing beside you.",
  },
  {
    id: "perspective",
    label: "One point",
    kicker: "02  ·  One point",
    title: "Everything meets.",
    body: "A stop ahead is the same kind of page as this one — a title on the left, a note on the right — only smaller, because it is farther down the same floor.",
  },
  {
    id: "stations",
    label: "Stations",
    kicker: "03  ·  Stations",
    title: "It stays where you leave it.",
    body: "Nothing is replaced when you walk on. The room keeps every page. Turn around and the last one is still there, full size, once you are standing in front of it.",
  },
  {
    id: "junction",
    label: "Junction",
    kicker: "04  ·  Junction",
    title: "The room divides.",
    body: "On your right the floor splits. That corridor is a branch: its own path, tied to this one. Step onto it while you are close. It is two rooms long, and it remembers the way back.",
  },
  {
    id: "input",
    label: "Input",
    kicker: "05  ·  Input",
    title: "Whatever used to scroll.",
    body: "The wheel and the trackpad walk you forward. Arrow keys step from stop to stop. A swipe is the wheel without a mouse. Click any page you can still see, and you go to it.",
  },
  {
    id: "horizon",
    label: "Horizon",
    kicker: "06  ·  Horizon",
    title: "You do not arrive.",
    body: "The point stays a point. There is a direction to face, and the writing is simply what the room holds out to you on the way there.",
  },
];

export const annexStops: Stop[] = [
  {
    id: "alcove",
    label: "Alcove",
    kicker: "A1  ·  Alcove",
    title: "A quieter measurement.",
    body: "A branch is a path with a parent. From here the long room is behind you. The core never sees this HTML. It only knows how far you are from each stop, and where that stop sits on the screen.",
    tone: "annex",
  },
  {
    id: "wayback",
    label: "Way back",
    kicker: "A2  ·  Way back",
    title: "Turn around.",
    body: "Walk backward off the start of a branch, or press Escape. You are set down at the junction you left. The annex stays where it was, ready to be entered again.",
    tone: "annex",
  },
];

export function galleryConfig() {
  const path = (id: string, stops: Stop[]) => ({
    id,
    nodes: stops.map((stop) => ({ id: stop.id, label: stop.label })),
  });
  return {
    rootPath: "main",
    paths: [path("main", mainStops), path("annex", annexStops)],
    branches: [
      {
        id: "to-annex",
        label: "The Annex",
        fromPath: "main",
        fromNode: "junction",
        toPath: "annex",
        side: "right",
      },
    ],
  };
}

export function galleryContent(): Record<string, NodeContent> {
  const content: Record<string, NodeContent> = {};
  for (const stop of [...mainStops, ...annexStops]) {
    content[stop.id] = {
      left: leftPanel(stop),
      right: rightPanel(stop),
    };
  }
  return content;
}

function leftPanel(stop: Stop): HTMLElement {
  const el = document.createElement("div");
  if (stop.tone === "annex") el.classList.add("tone-annex");
  const kicker = document.createElement("p");
  kicker.className = "kicker";
  kicker.textContent = stop.kicker;
  const title = document.createElement("h2");
  title.id = `${stop.id}-title`;
  title.textContent = stop.title;
  el.append(kicker, title);
  el.setAttribute("aria-labelledby", title.id);
  return el;
}

function rightPanel(stop: Stop): HTMLElement {
  const el = document.createElement("div");
  if (stop.tone === "annex") el.classList.add("tone-annex");
  const body = document.createElement("p");
  body.textContent = stop.body;
  el.append(body);
  el.setAttribute("aria-labelledby", `${stop.id}-title`);
  return el;
}
