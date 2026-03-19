# Release Process

- Update version on root Cargo.toml
- Make a PR
- Wait for PR to be merged
- Pull changes once merged
- Run `git tag v<version>`
- Run `git push --tags`. This will test, build and publish the version if successful.
- Go to the [releases page](https://github.com/starkware-libs/cairo_native/releases), edit the latest release, and click `Generate release notes`.
