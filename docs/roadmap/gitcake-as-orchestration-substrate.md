# Roadmap note: gitcake as a multi-agent orchestration substrate

Status: exploratory / not yet decided (not an ADR). Captured 2026-06-21.

## Context

This note was written from the perspective of an actual user of a multi-agent
orchestrator (`dot-agent-deck`), where an "orchestrator" agent coordinates worker
agents (coder / reviewer / auditor / release) through a delegate → work-done loop.
The question: **would gitcake be a useful tool for that orchestrator to structure
and coordinate tasks — especially during project initialization — and what is
missing?**

The interesting finding is that gitcake's data model is already noticeably
agent-aware, so the answer is "yes, as the structure & state backbone — but not
yet as the coordination engine."

## What already fits (verified against `core/src/models/task.rs`)

The `Task` ("slice") model already carries orchestration-relevant fields:

- `ai_flagged: bool` — "set by AI when it needs human input to continue." Maps
  directly to a human-in-the-loop checkpoint (e.g. orchestrator pausing for the
  user to validate, or a blocking finding that needs a human decision).
- `order: Option<u32>` — "optional sequence number for AI-planned work ordering."
- `parent_id: Option<String>` — subtasks grouped under a parent slice.
- `cake_id: Option<String>` — epic-level grouping.
- `bites` / `crumbs` — checklist items and notes attached to a slice.

Combined with git-backed storage (durable, versioned, shared) and the MCP server
(programmatic read/write by agents), the mapping to an orchestrator's planning job
is strong:

| Orchestrator need                              | gitcake primitive                  |
|------------------------------------------------|------------------------------------|
| Decompose project → features → tasks → subtasks| cake → slice → `parent_id` → bites |
| Sequence the plan                              | `order`                            |
| Pause for human validation                     | `ai_flagged`                       |
| Durable, shared task state                     | git-backed storage                 |
| Drive it from an agent                         | MCP tools                          |

For **project initialization** (greenfield decomposition into a structured,
versioned, assignable task tree) gitcake is already a good fit today. The gaps
below bite harder during *execution* (the review/rework loop) than during
*planning*.

Concrete near-term win: an orchestrator currently keeps task briefs and worker
reports as loose scratch files (e.g. `.dot-agent-deck/*.md`). Those could instead
live in gitcake slice bodies — versioned, grouped, queryable — rather than as
ad-hoc files.

## What's missing — priority order

### 1. Dependency graph, not just linear `order` (highest leverage)
Orchestration is a DAG with fan-out / fan-in:

```
coder ──▶ (reviewer ∥ auditor) ──▶ resolve findings ──▶ release
```

`order: u32` is a single lane; it cannot express "D is blocked by *both* B and C"
or "these two run in parallel." A `blocked_by: Vec<String>` (or equivalent edge
model) is needed for the core shape of coordination to be representable.

### 2. Agent-role assignee + a dispatch bridge
`owner` is a human user folder. Missing:
- A notion of role-as-assignee (coder / reviewer / auditor / release), distinct
  from a human owner.
- A bridge between gitcake state and the orchestrator's run loop: **assigning a
  slice should be able to trigger a delegation**, and a worker's **work-done
  should flip the slice to done**. Without this, the orchestrator double-books —
  gitcake for structure, the orchestrator's own channel for the actual run. This
  integration is where gitcake stops being a notepad and becomes the substrate the
  orchestrator runs *from*.

### 3. Review / finding / rework lifecycle
A large fraction of coordination is: findings (blocking vs non-blocking) → rework
rounds. Today's `open / in-progress / done` (+ `ai_flagged`) cannot represent
"blocked on review," "in rework," round/iteration history, or "this finding
*blocks* its parent's done." Findings want to be first-class child slices that
gate their parent.

### Smaller follow-ons
- Richer statuses: `in-review`, `blocked`, `awaiting-validation`.
- Confirm whether `crumbs` can serve as a per-slice activity log for worker
  reports / round notes.

## Recommendation

gitcake is already a strong **planning/structure** layer for project init —
better than a generic tracker because it is git-native, AI-aware (`ai_flagged`,
`order`, `parent_id`), and MCP-drivable. To become a real **orchestration
substrate**, land, in order: (1) dependency edges, (2) the assign↔delegate /
done↔work-done bridge, (3) the finding/rework lifecycle.

Note: the MCP currently exposes only part of `core` (no cakes, delete, backlog
move/claim, or pull) and uses "slice" terminology that should be reconciled with
CONTEXT.md before the API is frozen. Any orchestration-substrate work should ride
on top of an MCP parity + naming pass. See the deferred MCP note.

## Architecture Designer role + decision lifecycle

The whole multi-agent model rests on one fact: workers cold-start with no memory,
so the durable context they read *is* the system. An **Architecture Designer**
role is the missing *producer* of that context. Instead of an orchestrator (or a
human) hand-writing briefs, this role generates the canonical, structured starting
point that every downstream agent — orchestrated *or* standalone — reads to "know
where to work from."

gitcake's substrate for that output already exists (`docs/adr/`, a PRD concept via
the `/prd-update-progress` workflow, cakes → slices → `parent_id`/`order`). The
role is the missing producer, not a missing store.

### Output contract: a "bootstrapped gitcake repo"

```
PRD (what / why / scope / non-goals / success criteria)
  └─ ADRs (load-bearing decisions, with rationale + alternatives-rejected)
       └─ cakes (epics)
            └─ slices (tasks) — ordered, with parent_id,
                 EACH linked to its justifying ADR / PRD section
```

The load-bearing piece is the **slice → justifying-decision link**. "Knowing where
to work from" is not having ADRs; it is the *traceability* between a decision and
the task that implements it. Without that edge there is a pile of docs and a pile
of tasks with no connective tissue. The current model has `cake_id` and
`parent_id` but no "implements / justified-by" edge to a decision document — that
is the key model gap this role exposes.

Design the **artifact contract first** (what a bootstrapped repo *is*), then the
agent. The role should be runnable in two modes with identical output:
- **(a) Orchestrator worker** — an `architect` role the orchestrator delegates to
  *first*, then executes the plan it produces.
- **(b) Standalone / pre-orchestration** — a human bootstraps a repo, then hands
  the populated gitcake repo to an orchestrator, or works solo with a standalone
  AI reading it. (The "standalone AI knows where to work from" case.)
Only the trigger differs; the agent is replaceable, the contract is durable.

### Hard requirements (these are where the design earns its keep)

1. **ADR vs PRD have different lifecycles.** A PRD is mutable (scope evolves). An
   ADR is *immutable once accepted* — superseded, never edited. The role must not
   conflate them or silently rewrite an accepted ADR. This is not hypothetical:
   during the 2026-06-21 navbar/keymap work a coder agent silently rewrote ADRs
   0001 and 0003 (reverted). Treat accepted ADRs as **append-only / supersede-only**,
   enforced in the model / MCP — not merely by prompt.

2. **Architecture output lands `proposed`, never auto-`accepted`.** Decisions are
   the worst place for an AI to silently commit. Use `ai_flagged` as the gate:
   Architect drafts → flags for human → human accepts/amends → only then canonical.
   This requires a **decision lifecycle** (`proposed → accepted → superseded`), a
   new dimension on top of the slice `open → in-progress → done` lifecycle and the
   richer-status gap above.

3. **Decision altitude / anti-over-specification.** The classic failure of an
   upfront-architecture agent is over-deciding — pinning choices that should stay
   open, producing brittle scaffolding. Discipline: decide only what is
   load-bearing / blocking; defer the rest explicitly (YAGNI for decisions).

4. **The plan needs an adversarial review pass before acceptance.** A decomposition
   is a hypothesis. Check: does every PRD requirement map to a cake/slice? orphan
   requirements? circular dependencies (needs gap #1, dependency edges)? unjustified
   tasks? This is the planning analogue of running reviewer + auditor over code.

### Relationship to the rest of this note

This role is the *producer* for the substrate described above. It also adds a new
dimension — a **decision lifecycle** — and sharpens two existing gaps:
- gap #1 (dependency edges) is a prerequisite for the plan-review cycle check;
- gap #3 (review/finding/rework lifecycle) generalizes to also cover the
  `proposed → accepted → superseded` decision states.

Recommended order: lead with the **artifact contract** + the slice→decision link,
enforce ADR append-only/supersede in the model, gate output behind the
`proposed → accepted` lifecycle, then add the plan-review pass.

### Researcher role

Research is not a front-loaded phase; it is a **service invoked at two distinct
points** in the planning pipeline. Missing the second point is the common mistake.

```
exploration ──▶ PRD ──▶ ADRs ──▶ cakes/slices
     ▲                    ▲
  Researcher          Researcher        (same role, two jobs)
 (survey the         (ground each
  problem space)      decision's options)
```

1. **Upstream of the PRD — exploration.** Prior art, comparable systems, domain
   constraints, known pitfalls, candidate requirements. Open-ended scan that
   narrows into the PRD.
2. **Inside PRD → ADR — per-decision grounding.** The higher-leverage, more often
   missed point. An ADR's quality lives in its "alternatives considered + why
   rejected." Without research that section reflects only the Architect's training
   priors; with it, each load-bearing decision gets a grounded, citable option
   space. The Researcher feeds the Architect's *evidence*, not just the PRD.

Concrete argument for the role over Architect-priors alone: **freshness.** A
model's knowledge has a training cutoff; decisions about libraries / tools /
versions go stale by default. A researcher with live access is how you avoid
stale-by-training ADRs.

#### Discipline (where it earns its keep)

1. **Separation of powers — supplies, never decides.** Output is a *cited briefing*
   (options, evidence, tradeoffs), not a recommendation that silently becomes
   canonical. The decider stays Architect + human, behind the same `ai_flagged`
   gate as architecture output.
2. **Requirements come from the stakeholder, not the web.** The failure mode is a
   researcher fabricating plausible-but-wrong requirements (hallucinated scope).
   Its job in requirements-collection is to **generate the questions to put to the
   human** and surface *candidate* requirements — not to invent the product's
   intent. (Mirrors the deep-research discipline: if the ask is underspecified, ask
   clarifying questions before researching.)
3. **Citations / adversarial verification.** Research feeding load-bearing
   decisions must be verifiable — unsourced claims poison ADRs. Standard: fan out,
   adversarially verify claims, synthesize a *cited* report.
4. **Decision-driven scope (pull, not push).** Research expands without bound if
   unscoped. The Architect identifies a decision needing grounding → spawns a
   *scoped* brief → gets evidence back. Not a giant upfront survey. Mirrors the
   Architect's decision-altitude discipline.

#### Artifact: the research brief

- **New artifact type**, traceably linked: `ADR → research-brief-it-is-grounded-in`,
  `PRD section → exploration-note`. Extends the same traceability spine
  (slice → decision → evidence).
- **Freshness property.** Unlike an accepted ADR (immutable), a research brief is
  *point-in-time*: it carries an **as-of date + source list** header so a later
  supersession knows whether to re-research. Research reflects what was true when
  written.
- **Two modes, mirrored in tooling:** *external* (web / prior art — e.g. a
  deep-research harness) vs *internal* (existing codebase / constraints — e.g. a
  read-only Explore agent). Greenfield init leans external; evolving an existing
  repo leans internal.

#### Resulting role separation

```
Researcher (cited evidence) ─▶ Architect (decides, drafts PRD/ADRs)
   ─▶ human (accepts via ai_flagged) ─▶ Orchestrator (executes)
```

Recommendation: model the Researcher as a two-point service (exploration *and*
per-decision grounding), enforce *supplies-but-never-decides*, and emit **dated,
cited research briefs** linked to the PRD/ADR they ground.
