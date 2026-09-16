# Documentation maintenance

The site is generated from this repository's public guides and validated evidence.
`documentation.json` selects source pages; `mkdocs.yml` owns navigation.
`target/site/` is reproducible output and remains untracked.

## Review checklist

- [ ] Start with the audience's installation or integration task.
- [ ] Verify package names, exact alpha versions, ownership, errors, and examples.
- [ ] Separate declared adoption, executed parity, source coverage, C contract, and performance.
- [ ] Keep partial, planned, excluded, pending, and unmeasured paths visible.
- [ ] Bind every reported measurement to its source, inputs, runner, and date.
- [ ] Preserve legal notices and the final Puhu/Pillow acknowledgements.
- [ ] Regenerate contracts and inventories through their Make targets.
- [ ] Remove superseded internal plans after preserving durable public guidance.
- [ ] Check local links, rendered HTML, keyboard use, narrow layouts, and search.

## Build and preview

```sh
make docs-setup
make docs-test docs-lint
make docs-examples
make docs-build
make docs-serve
```

Documentation uses Python 3.12.10 in CI. Dependencies are fully pinned with
hashes in `requirements-docs.txt`. Change the direct pins, run `make docs-lock`,
and review the resulting lock before upgrading tools. Normal site builds do
not install tools or rerun benchmarks.

`make check-docs` also verifies adoption counts, runtime source receipts,
coverage arithmetic, C-contract totals, and the historical benchmark ledger.
These measurements live in [Evidence](EVIDENCE.md), not copied README tables.
The parity and benchmark recorders update that canonical page.

## Publish

The Documentation workflow builds pull requests and deploys main through GitHub
Pages in this same repository. Configure Pages to use GitHub Actions and the
`github-pages` environment. No separate hosting repository or registry token
is involved. Successful main Benchmark artifacts can refresh the result page;
only validated data is imported, while executable site code comes from main.
Main deployments retain the latest successful main benchmark's unexpired data,
including after later documentation pushes. Benchmark events require their exact
run's artifact. Local/PR builds and runs with no available hosted artifact use
the committed snapshot as an explicit fallback. API failures fail the build.

Run `make repository-inventory` after the final file changes. Keep the docs
index's lifecycle map complete so contributors can find every public guide.
