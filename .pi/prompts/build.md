---

Study @DESIGN.md
Study @PLAN.md

Run the existing tests to confirm a green baseline.

Start at the top of the plan and select the first unfinished phase by document order (including In Progress), then work only on that phase.

Do not do the entire phase, just the next open section in the phase (e.g. if there is a Phase 1 with 1.1 and 1.2 done, and 1.3 and 1.4 todo, you would pick up and complete only 1.3).

Create a feature branch and build it using red/green TDD.

Stop when the section is complete. You must complete the section.

After completing the work, please start a review loop with the review_loop tool with fresh context and default iterations. Instruct each looping agent to commit after completing its review. It's important for the review loop to spend time thinking before acting.

After the review loop is complete, update @PLAN.md to check off the items that were completed in this section. Then create a PR. Do not reference the phase or section number in the commit or PR title, just describe what the changes accomplish.
