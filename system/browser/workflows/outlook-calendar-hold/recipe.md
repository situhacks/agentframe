# Outlook Calendar Hold

workflow_id: outlook-calendar-hold
status: learning
app_url: https://outlook.office.com/calendar/
approval_mode: human_review

## Job

Create an Outlook calendar hold or meeting invite from the controlled Edge work browser, including attendees, subject, optional body, and a reasonable available time when the Scheduler can surface one.

## Tool Routing

Use browser-harness in the controlled Edge Work Browser. Cursor remains the reasoning layer and must stop before `Send` unless the operator explicitly authorizes sending the invite.

## Calling Preconditions

If Outlook Calendar is not already authenticated, run `workflows/_shared/auth-handoff/recipe.md` with the Calendar URL and return here only when the calendar surface is ready.

## Demonstrated Path

1. Open Outlook Calendar in Work Browser Mode.
2. Click `New event`.
3. Fill `Add title` with the requested meeting subject.
4. Fill `Invite required attendees` with the best available name or email fragment, select the matching Outlook directory suggestion, then verify the resolved attendee pill/row before scheduling.
5. Add a short body/description when the request includes one.
6. Open `Scheduler`.
7. Prefer Outlook's `Time suggestions` controls when they clearly show all required attendees are free.
8. Choose a slot that fits the operator's working hours and the attendee time-zone context.
9. Return from Scheduler to the event form.
10. Review title, attendees, body, and chosen time.
11. Stop before `Send` unless the operator explicitly authorized sending the invite for this run.

## Scheduling Heuristics

- Operator timezone: PST / Pacific.
- Default operator working hours: 9 AM - 5 PM PST.
- Many teammates are in Toronto / EST. When attendees are likely EST, prefer slots that are not outside a normal EST workday.
- Outlook Calendar may show both `EST` and `PST` rows in the Scheduler/grid. Treat those as orientation labels, not controls.
- If Outlook offers suggested slots with text like `{attendee} is free`, prefer those over manual grid scrolling. Skip suggestions before the shared-hours start or after the shared-hours cutoff.
- When `Time suggestions` starts on an invalid option, use the visible suggestion list or `Selects the next time suggestion` to cycle until a slot clearly says every required attendee is free.
- If no clear suggested slot is visible, or the available-time search requires heavy scrolling/interpretation, stop and ask the operator to pick a time.

## Known Controls

- `New event` opens the compose event surface.
- `Add title` is the meeting subject field.
- `Invite required attendees` is the attendee entry field.
- Attendee suggestions appear as options after typing a name or email fragment. The typed text may be imperfect; the durable state is the resolved attendee suggestion, not the raw typed string.
- `Scheduler` opens the availability surface.
- Suggested time-slot controls may include labels like `Selects the next time suggestion` or `{date/time}. {attendee} is free`.
- `Back` returns from Scheduler to the event form.
- `Send` sends the invite and is an external-consequence action.

## Human Gate

Default behavior is prepare-only. Cursor must stop before `Send` unless the operator explicitly authorizes sending this specific invite.

Cursor must also stop and ask the operator to choose a time when:

- Scheduler does not show a clearly available suggested slot.
- Required attendees have ambiguous availability.
- The best slot is outside 9 AM - 5 PM PST.
- EST teammate availability appears to fall outside a normal Toronto workday.
- The request has a high-stakes attendee or ambiguous meeting duration.

## Promotion Notes

Do not promote yet. The durable form-fill path is clear, but scheduling still needs more examples. Run at least two more demos that vary attendee count, duration, and time-zone context before changing this recipe's status.
