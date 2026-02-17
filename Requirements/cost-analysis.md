# Ink Gateway - API Cost Analysis (Simplified)

## Executive Summary
**True API cost to generate a 200-page novel via Ink Gateway: $0.04 to $9.04 depending on model choice.**

No setup costs, infrastructure costs, or labor included — just the raw API spend.

---

## Assumptions
- **Target:** 200 pages manuscript
- **Generation pace:** 5 pages per nightly session = 40 sessions total
- **Per-session context:**
  - Last 5 pages (with your inline comments)
  - Summary of previous 20 pages (condensed)
  - Global outline (plot structure)
  - Chapter outline (section guide)
  - System prompt (voice, tone, world rules)

---

## Token Calculation (Per Session)

| Component | Words | Tokens | Notes |
|-----------|-------|--------|-------|
| Last 5 pages + comments | 1,250–1,500 | 1,625–1,950 | Full context, recent work |
| Summary (20 prev pages) | 5,000 | 2,000 | Condensed recap (every 4x compression) |
| Global outline | 100–150 | 400 | Plot structure, beat guide |
| Chapter outline | 75–100 | 250 | Specific chapter goals |
| System prompt | 75–100 | 200 | Voice, tone, style rules |
| SYSTEM_PROMPT.md (world/chars) | 150–200 | 300 | World consistency rules |
| **Total Input** | **~7,650** | **~4,775** | Per session input |
| **Generated Output** | ~1,250 (5 pages) | ~2,250 | The new content |
| **Per Session Total** | **~8,900** | **~7,025** | In + Out combined |

---

## API Cost Per Session (40 Sessions Total)

### Claude Opus
- **Input:** $15 / 1M tokens
- **Output:** $45 / 1M tokens
- **Per session:** (4,775 × $15/1M) + (2,250 × $45/1M) = $0.0718 + $0.1013 = **$0.1731**
- **40 sessions:** **$6.92**
- **Per page:** **$0.0346**

### Claude 4.5 Sonnet
- **Input:** $3 / 1M tokens
- **Output:** $15 / 1M tokens
- **Per session:** (4,775 × $3/1M) + (2,250 × $15/1M) = $0.0143 + $0.0338 = **$0.0481**
- **40 sessions:** **$1.93**
- **Per page:** **$0.0096**

### Claude 3.5 Sonnet
- **Input:** $3 / 1M tokens
- **Output:** $15 / 1M tokens
- **Per session:** **$0.0481**
- **40 sessions:** **$1.93**
- **Per page:** **$0.0096**

### GPT-4 Turbo
- **Input:** $10 / 1M tokens
- **Output:** $30 / 1M tokens
- **Per session:** (4,775 × $10/1M) + (2,250 × $30/1M) = $0.0478 + $0.0675 = **$0.1153**
- **40 sessions:** **$4.61**
- **Per page:** **$0.0231**

### Gemini Flash (budget option)
- **Input:** $0.075 / 1M tokens
- **Output:** $0.30 / 1M tokens
- **Per session:** (4,775 × $0.075/1M) + (2,250 × $0.30/1M) = $0.00036 + $0.00068 = **$0.00104**
- **40 sessions:** **$0.04**
- **Per page:** **$0.0002**

---

## Comparison Table (200 Pages)

| Model | Per Session | 40 Sessions Total | Per Page |
|-------|-------------|-------------------|----------|
| **Gemini Flash** (budget) | $0.001 | **$0.04** | $0.0002 |
| **Claude 3.5 Sonnet** (quality) | $0.048 | **$1.93** | $0.0096 |
| **Claude 4.5 Sonnet** (premium) | $0.048 | **$1.93** | $0.0096 |
| **GPT-4 Turbo** (expensive) | $0.115 | **$4.61** | $0.0231 |
| **Claude Opus** (top-tier) | $0.173 | **$6.92** | $0.0346 |

---

## Cost Scaling (Multiple Books)

| Books | Model | Total API Cost |
|-------|-------|-----------------|
| 1 book (200 pages) | Claude 3.5 Sonnet | $1.93 |
| 3 books | Claude 3.5 Sonnet | $5.79 |
| 5 books | Claude 3.5 Sonnet | $9.65 |
| 1 book | Gemini Flash | $0.04 |
| 5 books | Gemini Flash | $0.20 |

---

## Recommendation

**Best value for fiction quality:** **Claude 3.5 Sonnet or 4.5 Sonnet**
- Cost: $1.93 per 200-page book
- Quality: Excellent for creative writing
- Per page: Less than 1 cent

**Budget alternative:** **Gemini Flash**
- Cost: $0.04 per 200-page book
- Quality: Adequate, but less nuanced for prose
- Per page: 0.02 cents

**Premium option:** **Claude Opus**
- Cost: $6.92 per 200-page book
- Quality: Absolute best in class
- Per page: 3.46 cents

---

## The Reality

Writing a 200-page novel via Ink Gateway costs:
- **~$2** with Claude (best for fiction)
- **~$0.04** with Gemini (ultra-cheap)
- **~$7** with Opus (overkill, but available)

This is **purely API spend**. No labor, no infrastructure, no setup recovery. Just the raw compute cost to generate prose.
