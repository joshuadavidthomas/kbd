Study @DESIGN.md
Study @PLAN.md

Run the existing tests to confirm a green baseline.

Start at the top of the plan and select the first unfinished phase by document order (including In Progress), then work only on that phase.

Do not do the entire phase, just the next open section in the phase (e.g. if there is a Phase 1 with 1.1 and 1.2 done, and 1.3 and 1.4 todo, you would pick up and complete only 1.3).

Create a feature branch and build it using red/green TDD. You **MUST** run the tests before starting to prove RED and run the tests after to prove GREEN.

Before starting work, load the thinking-in-rust skill and any other relevant rust skill to the phase of work you are implementing.

Do not remove TODO comments unless you are actually implementing what that TODO says. TODO comments preserve important project context and must stay in place until completed. If you remove or rewrite a TODO without implementing it, you must explicitly explain why.

Stop when the section is complete. You must complete the section.

After completing the work, start the `review_loop` tool once with fresh context and default iterations. Instruct each looping agent to read the relevant rust skills before working and to commit after completing its review. It's important for the review loop to spend time thinking before acting.
Treat `review_loop` as fire-and-forget: **call it once and stop**. Do not repeatedly poke, re-trigger, or micromanage it. Do not stop it manually after one iteration. Let it run until the tool itself reports completion (for example, no issues found or max iterations reached).

After the review loop is complete, update @PLAN.md to check off the items that were completed in this section. Run `just clippy` and `just fmt`. Then create a PR. Do not reference the phase or section number in the commit or PR title, just describe what the changes accomplish.
