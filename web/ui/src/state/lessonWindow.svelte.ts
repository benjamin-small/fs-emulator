import { parseStoredPosition, type Point } from "../core/floating";

const POS_KEY = "fs-explorer.lesson.pos";
const MIN_KEY = "fs-explorer.lesson.minimized";

/**
 * Where the user left the floating Lesson card. `null` until they move it, which means
 * the default spot (LessonPanel computes it from the layout). DOM-free like the other
 * stores: clamping needs the card's size, so it happens in the component.
 */
export class LessonWindowStore {
  pos = $state<Point | null>(parseStoredPosition(readStored(POS_KEY)));
  /** Minimized: only the bar and the Prev/Next/Close row show, so the card takes little
   *  room while it stays floating and movable. Remembered like the position. */
  minimized = $state(readStored(MIN_KEY) === "1");

  toggleMinimized() {
    this.minimized = !this.minimized;
    try {
      localStorage.setItem(MIN_KEY, this.minimized ? "1" : "0");
    } catch {
      // Private mode or a full quota: the choice lasts until the next reload.
    }
  }

  /** Follow a drag or a nudge; nothing is written until `commit()`. */
  setPos(p: Point) {
    this.pos = p;
  }

  /** End of a drag or a nudge: remember the spot across reloads. */
  commit() {
    if (!this.pos) return;
    try {
      localStorage.setItem(POS_KEY, JSON.stringify(this.pos));
    } catch {
      // Private mode or a full quota: the spot lasts until the next reload.
    }
  }
}

function readStored(key: string): string | null {
  try {
    if (typeof localStorage === "undefined") return null;
    return localStorage.getItem(key);
  } catch {
    return null;
  }
}

export const lessonWindow = new LessonWindowStore();
