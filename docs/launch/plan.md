# Beta launch plan

The goal is **10 real testers first**, then 100 active users. Ten people who
actually install it and report what breaks is worth more than a thousand stars,
and it is the only milestone that can be measured honestly.

Nothing in this file has been posted. Every draft is in `announcements.md` and
waits for your approval.

## Order of operations

| # | Step | Why this order |
| --- | --- | --- |
| 1 | Release published, checksums verified, README correct | Everything else points at it |
| 2 | Welcome discussion pinned | First thing a visitor reads |
| 3 | One post in the smallest, most relevant community | Get the obvious bugs out before a big audience |
| 4 | Fix what that post surfaces | A launch thread full of crash reports is a bad look |
| 5 | Second community, a different day | Never crosspost the same text on the same day |
| 6 | Hacker News, only when the first two went well | It is one shot and it is unforgiving |
| 7 | Rust communities | The audience that might contribute code |

Do not open with Hacker News. It is the highest-traffic, lowest-patience
audience, and a beta with known Linux gaps will be read as an overclaim.

## Rules that matter

These are the rules I am working from. Confirm them against each community's
current sidebar or guidelines before posting — they change, and the moderators
enforce them.

* **One post per community, from your own account, as the author.** No alt
  accounts, no posting on someone else's behalf, no asking friends to upvote. A
  single coordinated upvote can get the account and the project banned.
* **Disclose that you are the author** in the first line. Every community
  tolerates a maker's post; almost none tolerate a maker pretending to be a
  satisfied user.
* **Read the self-promotion rule.** Some subreddits only allow project posts on
  specific days or in weekly threads; some require a minimum account age or
  karma. If a community says no, respect it and skip it.
* **Answer every comment for the first few hours.** The posts that fail are the
  ones where the author disappears.
* **Do not argue with criticism.** "It's a beta, here is what's missing, thank
  you" ends a thread; defending a gap does not.
* **Never buy stars, reviews or upvotes**, and never state a user number you
  cannot point at.

## What to lead with

Ranked by what is actually true and actually unusual:

1. **Two real engine shells, one shared core.** WebKitGTK on Linux and WebView2
   on Windows, with the browser logic platform-neutral in Rust. No Electron, no
   Chromium bundle: the Linux binary is well under a megabyte and uses the system
   engine.
2. **No telemetry, and it is checkable.** No analytics, no crash pings, no update
   checks. The source and the CI are public; `docs/PRIVACY.md` states what the
   protections do and what they do not.
3. **Privacy claims with a probe behind them.** A two-session harness runs on
   every push and asserts 18 invariants, including that canvas noise is stable
   within a session and different across sessions.
4. **An honest readme.** Known bugs, measured memory and startup, and a list of
   what is not built — in the repository, not buried.

Do not lead with "lightweight" as a slogan. Measured at 306–474 MB it is not yet
lightweight, and someone will check.

## Tracking

* **Testers:** count distinct people who post in Discussions or open an issue, not
  downloads. Downloads include CI, mirrors and the curious.
* **Milestone 1 (10 testers):** a GitHub milestone of that name, closed when ten
  distinct humans have reported something they actually ran.
* **Milestone 2 (100 active users):** only measurable if a user tells you they are
  using it; there is no telemetry, by design. Treat any number as "people who said
  so".
* **Feedback triage:** bug template → label `needs-triage` → reproduce → either
  `confirmed` with a milestone, or `cannot-reproduce` with what is missing.

## Success looks like

Ten people install it, three of them hit the same bug, one of them opens a pull
request. That is a better outcome than 500 stars and silence.
