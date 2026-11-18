# Publishes this app as a branch of your fork of rallegretti/PlanCraft.
# Needs: git and the GitHub CLI (winget install GitHub.cli), signed in with `gh auth login`.
# Run from the app folder:  powershell -ExecutionPolicy Bypass -File tools\publish.ps1
param([string]$Branch = "ribbon-edition")
$ErrorActionPreference = "Stop"
if (-not (Get-Command gh -ErrorAction SilentlyContinue)) { throw "Install the GitHub CLI first: winget install GitHub.cli, then run: gh auth login" }
gh auth status | Out-Null
if (-not (git config user.name)) { throw "Set your git name first: git config --global user.name ""Your Name""" }
if (-not (git config user.email)) { throw "Set your git email first: git config --global user.email you@example.com" }

# 1. fork (does nothing if the fork already exists)
gh repo fork rallegretti/PlanCraft --clone=false
$me = gh api user --jq .login
$fork = "https://github.com/$me/PlanCraft.git"

# 2. commit this folder on its own branch (the .gitignore keeps build output, screenshots and
#    machine-specific settings out)
if (-not (Test-Path .git)) { git init -q }
git checkout -q -B $Branch
git add -A
git commit -q -m "PlanCraft ribbon edition: a Rust project scheduler driven by one command registry"
git remote remove fork 2>$null
git remote add fork $fork

# 3. push the branch to your fork
git push -u fork $Branch
Write-Output "Published: https://github.com/$me/PlanCraft/tree/$Branch"
