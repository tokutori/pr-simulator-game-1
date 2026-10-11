import type { GameUiFrame } from "../generated/game-ui-frame.js";
import type { GameUiView } from "../generated/game-ui-view.js";

export function mergeGameUiFrame(view: GameUiView, frame: GameUiFrame): GameUiView {
  if (view.view_revision !== frame.view_revision) throw new RangeError("Shared UI frame belongs to a superseded view");
  const ranges = new Map(frame.ranges.map((range) => [range.id, range.value]));
  const readouts = new Map(frame.readouts.map((readout) => [readout.id, readout.value]));
  const buttons = new Map(frame.button_labels.map((button) => [button.id, button.label]));
  const cursor = frame.chart_cursor;
  return Object.freeze({ ...view, selections: frame.selections, hud: frame.hud,
    panels: Object.freeze(view.panels.map((panel) => Object.freeze({ ...panel,
      controls: Object.freeze(panel.controls.map((control) => {
        const content = control.content;
        if (content.kind === "range" && ranges.has(control.id)) {
          return Object.freeze({ ...control, content: Object.freeze({ ...content, value: ranges.get(control.id) ?? content.value }) });
        }
        if (content.kind === "status" && readouts.has(control.id)) {
          return Object.freeze({ ...control, content: Object.freeze({ ...content, value: readouts.get(control.id) ?? content.value }) });
        }
        if (content.kind === "button" && buttons.has(control.id)) {
          return Object.freeze({ ...control, label: buttons.get(control.id) ?? control.label });
        }
        if (content.kind === "chart" && cursor !== null && cursor.id === control.id) {
          const vectors = cursor.selected_wind_vector === null ? content.plot.vectors
            : Object.freeze([cursor.selected_wind_vector, ...content.plot.vectors.slice(1)]);
          return Object.freeze({ ...control, content: Object.freeze({ ...content,
            plot: Object.freeze({ ...content.plot, cursor_x: cursor.cursor_x, cursor_points: cursor.cursor_points, vectors }) }) });
        }
        return control;
      }))
    })))
  });
}
