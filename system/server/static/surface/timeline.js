// Swimlane (Timeline view) renderer. Full implementation lands in Task 7;
// this stub keeps the calendar shell importable in the meantime.

export function renderTimeline(board, _projects, _opts) {
  board.replaceChildren();
  const note = document.createElement('div');
  note.className = 'calendar-empty';
  note.textContent = 'Timeline view loading…';
  board.append(note);
}
