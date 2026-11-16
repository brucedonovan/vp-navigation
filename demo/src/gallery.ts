import type { NodeContent } from "../../packages/dom/src/index";

export interface ViewCopy {
  id: string;
  emphasis: number;
  column?: number;
  row?: number;
  colSpan?: number;
  rowSpan?: number;
  kicker?: string;
  title?: string;
  body?: string;
}

export interface Stop {
  id: string;
  label: string;
  orientation: "horizontal" | "vertical";
  grid?: { columns?: number; rows?: number };
  tone?: "alley";
  views: ViewCopy[];
}

export const mainStops: Stop[] = [
  {
    id: "threshold",
    label: "Threshold",
    orientation: "horizontal",
    views: [
      {
        id: "whole",
        emphasis: 100,
        kicker: "01  ·  One view",
        title: "The whole front.",
        body: "A stop can hold a single view. It takes the center — the full page you are standing in — and fades where it is when you walk on.",
      },
    ],
  },
  {
    id: "perspective",
    label: "One point",
    orientation: "horizontal",
    views: [
      {
        id: "left",
        emphasis: 50,
        kicker: "02  ·  Even split",
        title: "Side by side.",
      },
      {
        id: "right",
        emphasis: 50,
        body: "Two views at 50 emphasis share the front equally. Walk on and they burst outward and fade.",
      },
    ],
  },
  {
    id: "stations",
    label: "Stations",
    orientation: "vertical",
    views: [
      {
        id: "top",
        emphasis: 50,
        kicker: "03  ·  Up and down",
        title: "One above the other.",
      },
      {
        id: "bottom",
        emphasis: 50,
        body: "The same half share, turned. These two burst up and down, and fade, as you pass.",
      },
    ],
  },
  {
    id: "junction",
    label: "Junction",
    orientation: "horizontal",
    views: [
      {
        id: "half",
        emphasis: 50,
        kicker: "04  ·  Emphasis",
        title: "Half.",
        body: "This column holds half the front. The Alley opens to the right.",
      },
      {
        id: "thirty",
        emphasis: 30,
        title: "Thirty.",
        body: "A narrower share of the same stop.",
      },
      {
        id: "twenty",
        emphasis: 20,
        title: "Twenty.",
        body: "A row leaves the same way as any other grid.",
      },
    ],
  },
  {
    id: "lattice",
    label: "Lattice",
    orientation: "horizontal",
    grid: { columns: 3, rows: 2 },
    views: [
      {
        id: "wide",
        emphasis: 60,
        column: 1,
        row: 1,
        colSpan: 2,
        kicker: "05  ·  Any grid",
        title: "Columns, rows, spans.",
        body: "Name the cells. What is left fills in. The whole front is still one stop.",
      },
      {
        id: "side",
        emphasis: 40,
        column: 3,
        row: 1,
        rowSpan: 2,
        title: "A tall cell.",
        body: "This one spans both rows.",
      },
      {
        id: "low",
        emphasis: 30,
        title: "Then it bursts.",
      },
      {
        id: "mid",
        emphasis: 30,
        body: "Walk on. Every cell leaves the center and fades.",
      },
    ],
  },
  {
    id: "input",
    label: "Input",
    orientation: "vertical",
    views: [
      {
        id: "wheel",
        emphasis: 40,
        kicker: "06  ·  Input",
        title: "Wheel.",
        body: "Walks you along the path.",
      },
      {
        id: "arrows",
        emphasis: 30,
        title: "Arrows.",
        body: "Step from stop to stop.",
      },
      {
        id: "click",
        emphasis: 30,
        title: "Click.",
        body: "Any view you can still see.",
      },
    ],
  },
  {
    id: "horizon",
    label: "Horizon",
    orientation: "horizontal",
    views: [
      {
        id: "end",
        emphasis: 100,
        kicker: "07  ·  Horizon",
        title: "You do not arrive.",
        body: "The point stays a point. The views are only what the room holds out to you on the way there.",
      },
    ],
  },
];

export const alleyStops: Stop[] = [
  {
    id: "alcove",
    label: "Alcove",
    orientation: "horizontal",
    tone: "alley",
    views: [
      {
        id: "wide",
        emphasis: 60,
        kicker: "Alley  ·  Alcove",
        title: "Sixty.",
        body: "Emphasis is a share of the front, not a count of pages. This is the alley you turned into.",
      },
      {
        id: "rest",
        emphasis: 40,
        title: "Forty.",
        body: "The long room is behind you. This path remembers the way back.",
      },
    ],
  },
  {
    id: "wayback",
    label: "Way back",
    orientation: "horizontal",
    tone: "alley",
    views: [
      {
        id: "return",
        emphasis: 100,
        kicker: "Alley  ·  Way back",
        title: "Turn around.",
        body: "Walk backward off the start of a branch, or press Escape. You are set down at the junction you left.",
      },
    ],
  },
];

export function galleryConfig() {
  const path = (id: string, stops: Stop[]) => ({
    id,
    nodes: stops.map((stop) => ({
      id: stop.id,
      label: stop.label,
      orientation: stop.orientation,
      ...(stop.grid ? { grid: stop.grid } : {}),
      views: stop.views.map((view) => ({
        id: view.id,
        emphasis: view.emphasis,
        ...(view.column != null ? { column: view.column } : {}),
        ...(view.row != null ? { row: view.row } : {}),
        ...(view.colSpan != null ? { colSpan: view.colSpan } : {}),
        ...(view.rowSpan != null ? { rowSpan: view.rowSpan } : {}),
      })),
    })),
  });
  return {
    rootPath: "main",
    paths: [path("main", mainStops), path("alley", alleyStops)],
    branches: [
      {
        id: "to-alley",
        label: "The Alley",
        fromPath: "main",
        fromNode: "junction",
        toPath: "alley",
        side: "right",
      },
    ],
    // Every field is optional. See docs/configuration.md.
    projection: {
      spring: 0.3,
      springRange: 0.2,
      dwellMs: 280,
      frontWidth: 0.88,
      pageHeight: 0.58,
      turnExit: 2.6,
    },
  };
}

export function galleryContent(): Record<string, NodeContent> {
  const content: Record<string, NodeContent> = {};
  for (const stop of [...mainStops, ...alleyStops]) {
    content[stop.id] = {
      views: stop.views.map((view) => ({
        id: view.id,
        element: viewElement(stop, view),
      })),
    };
  }
  return content;
}

function viewElement(stop: Stop, view: ViewCopy): HTMLElement {
  const el = document.createElement("div");
  if (stop.tone === "alley") el.classList.add("tone-alley");
  if (view.kicker) {
    const kicker = document.createElement("p");
    kicker.className = "kicker";
    kicker.textContent = view.kicker;
    el.append(kicker);
  }
  if (view.title) {
    const title = document.createElement("h2");
    title.id = `${stop.id}-${view.id}-title`;
    title.textContent = view.title;
    el.append(title);
    el.setAttribute("aria-labelledby", title.id);
  }
  if (view.body) {
    const body = document.createElement("p");
    body.textContent = view.body;
    el.append(body);
  }
  return el;
}
