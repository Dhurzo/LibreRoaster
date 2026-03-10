import re

with open('.planning/PROJECT.md', 'r') as f:
    content = f.read()

new_current_state = """## Current Milestone: v4.1 Documentation Update

**Goal:** Update readme with new code status and functionality. Clean all the information outdated and update it.

**Target features:**
- Cleanup outdated info
- Recent changes (async changes, transport resilience)
- Build/Test instructions
"""

content = re.sub(r'## Current State\n\nv4\.0 shipped:.*?\n\n<details>', new_current_state + '\n<details>', content, flags=re.DOTALL)

with open('.planning/PROJECT.md', 'w') as f:
    f.write(content)

with open('.planning/STATE.md', 'r') as f:
    state_content = f.read()

new_state = """## Current Position

Phase: Not started (defining requirements)
Plan: —
Status: Defining requirements
Last activity: 2026-02-20 — Milestone v4.1 started
"""

state_content = re.sub(r'## Current Position.*?\n\n## Roadmap', new_state + '\n## Roadmap', state_content, flags=re.DOTALL)

with open('.planning/STATE.md', 'w') as f:
    f.write(state_content)
