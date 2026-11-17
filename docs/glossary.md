# Glossary

| Term | Meaning |
|---|---|
| TQA | Testing y Quality Assurance, the class this tool serves. |
| Annex | The Typst chapter that carries the dictionary, traceability, and evidence. |
| Dictionary | `tqa/casos.yaml`: use cases, test cases, steps, expected results, status. |
| Evidence | The run log, screenshots, and CI artifacts that show what happened. |
| Use case (UC) | `UC-<module>-<n>`, e.g. `UC-M02-01`. |
| Test case (TC) | `TC-<use case>-<n>`, e.g. `TC-M02-01-1`. |
| Run | One invocation of a suite, normalized into `TestRun`. |
| Suite | A group of tests within a run (a Minitest class, a Cargo module). |
| Green | No failures and no errors; skips do not break green. |
| Congruence | The dictionary and the tests agree: every row has a real test, every test has a row, statuses match. |
| M-01..M-08 | Thesis modules; Laburo currently serves M-01..M-03 (the web system). |
