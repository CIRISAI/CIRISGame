# CIRISGame — string registry (en).
# Single source for every announced surface: the unified ARIA-live region (§7.7),
# legend, caption strip, daily microcopy, replay toasts. Grade-5 reading level.
# NEVER use: rejected, invalid, cheat, verification failed, tampering.
# Pigment names only — custom steward nicknames stay strictly local.

## Rule + caption
caption-rule = Don't let your mesh reach seven.
rule-line-1 = Place stones.
rule-line-2 = Don't let your mesh hit seven.
rule-line-3 = Same seed for everyone today.
rule-line-4 = Lowest perma-dead wins.

## Legend / thinking (verb is shown in Verdigris italic)
steward-status = { $pigment } · { $verb }
verb-thinking = thinking
verb-weighing = weighing
verb-reading = reading
verb-pondering = pondering
verb-holding-breath = holding its breath
mesh-count = { $size }/7
status-alive = alive
status-eliminated = eliminated

## Atari + dispersal (announced)
aria-atari = { $pigment } mesh is at six. One more cell and it disperses.
aria-dispersal = { $pigment } mesh reached seven and dispersed. { $perma } perma-dead added.
aria-placed = { $pigment } placed a stone.
aria-board-minimap = Board minimap revealed. Twist or Shift plus Arrow to rotate.

## End screen
end-subline = Game { $game } · { $turns } turns · { $time }
end-federation-held = The federation held.
end-new-game = New game

## Daily seed
daily-today = Today — { $date }
daily-play = Play today
daily-scars = today: { $count } substrate scars
daily-next = next puzzle in { $time }
daily-ribbon = today: { $plays } plays — { $survivors } reached all-survivors

## Submit states (no accusation, ever)
submit-button = Submit today's score
submit-sending = Score sending.
submit-counted = Score counted.
submit-saved-local = Score saved on this device.
submit-network-full = Daily ribbon full for this network. Score saved here.
submit-retry = Will retry on next visit.

## Replay / share
replay-saved = Replay saved.
replay-shared = Replay saved. Receipt copied.

## Sound
sound-welcome = Sound off — tap to enable.

## Intro — click-through that teaches the one rule (§8.3 first-visit panel)
intro-tagline = Collapse is generative, not the end.
intro-screen-1-title = Place stones.
intro-screen-1-body = Four stewards take turns. On your turn you place one stone of your color on the lattice.
intro-screen-2-title = Don't let your mesh hit seven.
intro-screen-2-body = Your stones that touch make a mesh. When a mesh reaches seven cells it bursts and leaves dead cells behind.
intro-screen-3-title = Lowest perma-dead wins.
intro-screen-3-body = Each burst scatters perma-dead cells. Make the fewest. When all four reach zero, the federation holds.
intro-next = Next
intro-back = Back
intro-skip = Skip
intro-play = Play

## Setup wizard (§5.4 stewards drawer, §6.3 defaults)
wizard-title = Set up your game
wizard-step-of = Step { $step } of { $total }
wizard-back = Back
wizard-next = Next
wizard-start = Start game
wizard-step-players-title = Who is playing?
wizard-step-view-title-agent = Board view delivery
wizard-step-view-title-human = Accessibility
wizard-step-language-title = Language

## Player kinds + computer difficulty (§6.3)
player-human = Human
player-computer = Computer
player-agent = Agent
diff-easy = Easy
diff-medium = Medium
diff-hard = Hard
diff-brutal = Brutal

## Shared toggle words
toggle-on = On
toggle-off = Off

## View config — agent framing (§7 BoardView delivery)
view-graphics = Graphics
view-animation = Video / animation
view-format = Format
view-format-json = JSON
view-format-ascii = ASCII
view-format-png = PNG
view-format-animation = Animation
view-framerate = Frame rate
view-framerate-value = { $fps } fps
view-size = Image size
view-size-value = { $px } px

## View config — human / accessibility framing (same knobs, §6.7 / §7.7)
a11y-reduced-motion = Reduced motion
a11y-effects-quality = Effects quality
a11y-flat-view = Flat top-down view
a11y-screen-reader = Screen-reader announcements
a11y-captions = Captions
a11y-high-contrast = High contrast
a11y-colorblind = Colorblind emphasis
a11y-text-size = Text size
a11y-audio-mute = Mute sound
quality-low = Low
quality-medium = Medium
quality-high = High
text-size-small = Small
text-size-normal = Normal
text-size-large = Large

# Attract screen
play-now = Play Now
