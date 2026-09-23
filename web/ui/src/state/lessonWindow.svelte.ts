import { parseStoredPosition, type Point } from "../core/floating";

const POS_KEY = "fs-explorer.lesson.pos";

/**
 * Where the user left the floating Lesson card. `null` until they move it, which means
 * the default spot (LessonPanel computes it from the layout). DOM-free like the other
 * stores: clamping needs the card's size, so it happens in the component.
 */
export class LessonWindowStore {
  pos = $state<Point | null>(parseStoredPosition(readStored()));

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

function readStored(): string | null {
  try {
    if (typeof localStorage === "undefined") return null;
    return localStorage.getItem(POS_KEY);
  } catch {
    return null;
  }
}

export const lessonWindow = new LessonWindowStore();
