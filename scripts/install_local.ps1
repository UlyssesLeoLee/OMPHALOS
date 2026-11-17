$ErrorActionPreference = 'Stop'
cargo install --path . --force
if ($LASTEXITCODE -ne 0) { throw 'Rust build/install failed' }
Write-Host 'Edit PROJECT/.concept-guard/config.toml to point to the existing requirements, then run init and approve.'
