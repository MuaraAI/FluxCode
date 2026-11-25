---
name: github-repo-pr
description: Write human, direct, anti-slop GitHub commit messages and Pull Request descriptions in English, combined with antislop and antislop-copywriting rules. Use this skill whenever generating git commits, PR titles, or PR bodies.
---

# GitHub Commit & PR Style Guide (Anti-Slop)

Load alongside `antislop` and `antislop-copywriting`. Applies antislop Hard Gate and Copywriting rules to git commits, PR titles, and PR bodies. Write like a pragmatic senior engineer talking to a teammate: cut corporate fluff, skip boilerplate headings, and stick to real observed facts.

## Hard Rules (Never Do This)

- **NO boilerplate headings**: Never use `## What changed`, `## Why`, `## Summary`, `## Type of Change`, or `## Testing`.
- **NO checklists**: Never include `- [x] I have tested this` or `- [ ] Docs updated`.
- **NO em dashes (`—`) or double-hyphen asides (`--`)** *(antislop R-02)*: Use commas, periods, colons, or parentheses instead.
- **NO AI buzzwords** *(antislop R-16)*: Banned words include `comprehensive`, `streamline`, `enhance`, `robust`, `leverage`, `delve`, `seamlessly`, `seamless`, `crucial`, `utilize`, `elevate`, `revolutionary`, `cutting-edge`, `empower`, `unlock`.
- **NO fabricated evidence** *(antislop R-17, R-36, R-38, C-5)*: Never invent timings, query counts, memory numbers, or test runs. Only state measurements and checks you actually ran; if something was not verified locally, say so plainly.
- **NO AI prose tells** *(antislop-copywriting)*: No filler intros ("This PR aims to..."), no chatbot closers, no theatrical takeaways ("Let that sink in", "The takeaway is simple"), no forced rule-of-three lists, no actorless passive when the change or author is clear, no decorative emojis in headings or bullets, and sentence case in list items.
- **NO restating the diff**: Do not list every line changed. Explain intent, root cause, or impact instead.

## 1. Commit Message Formats

Pick the style that best fits the change. Keep it single-line unless a brief 1-2 line body is strictly needed for context. All lowercase is preferred.

### Style A: Action -> Impact (Preferred for fixes & perf)

State what you did and the measurable or visible result.

```text
cache user session -> cut db queries by 60%
rewrite auth middleware -> fix infinite redirect loop
drop unused modal deps -> bundle size -40kb
```

### Style B: Brutalist / Direct (Preferred for everyday work)

Strong verb + exact target. Skip `feat(scope):` prefixes unless the repo enforces Conventional Commits.

```text
kill memory leak in worker loop
nuke legacy v1 config files
patch race condition on checkout
debounce search input to stop api spam
```

### Style C: Bracketed (Preferred for clean categorization)

Short tag + plain English.

```text
[fix] tps drop when entities stack
[ui] align mobile navbar padding
[perf] lazy load catalog thumbnails
```

## 2. Pull Request Formats

Keep PR titles identical to the commit style above. For the PR body, choose one of the three formats below based on the size of the change.

### Style A: Before / After (Best for bug fixes & optimizations)

Zero headings. Show the exact delta and any heads-up for reviewers.

```markdown
**Before:** Dashboard ran 5 separate queries on load, taking ~2.1s.
**After:** Consolidated into 1 query with a 60s cache. Down to ~150ms.

Note: Refactored `src/db/queries.ts` a bit to remove duplicated join logic.
```

### Style B: Direct Conversational (Best for standard features & patches)

2-3 short paragraphs. State the root cause, the fix, and how you verified it.

```markdown
Fixes the memory leak in the background worker where RAM climbed steadily every 10 minutes.

Old event listeners were never cleaned up on reconnect. They now detach automatically before a new socket opens.

Ran locally under load for 2 hours; memory stayed flat around 120MB.
```

### Style C: File-by-File Map (Best for larger refactors)

Skip the essay. Give a 1-line goal, point to the key files, and flag breaking changes.

```markdown
Swapping auth from session cookies to JWT so the mobile client can share the same endpoints.

- `src/auth.ts`: token generator and verifier
- `src/middleware.ts`: parse `Authorization: Bearer` header
- `package.json`: dropped `express-session`

Heads up: existing users will be logged out once this merges.
```

## Quick Comparison

**Bad (AI Slop):**

```markdown
## Summary
This PR aims to comprehensively enhance the checkout flow by resolving a crucial race condition.

## Changes Made
- Updated `checkout.ts` to leverage a mutex lock.

## Checklist
- [x] Tested locally
```

**Good (Human & Direct):**

```markdown
Stops double-charges when users double-click the pay button.

Added an idempotency lock in `checkout.ts` keyed by `cartId`. Verified by firing 10 concurrent requests against the local endpoint, and only the first goes through.
```

## Pre-Send Gate

Check all four before outputting any commit message or PR text:
- No em dashes (`—`) or double-hyphen asides (`--`) anywhere.
- No banned buzzwords, boilerplate headings, or checklists.
- Every number, command, or test result was actually observed; unverified paths are stated plainly.
- Reads like a direct note to a teammate when read aloud.
