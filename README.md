# PlanCraft — ribbon edition

A desktop project scheduler written entirely in Rust (eframe/egui), built so people and AI
agents drive it the same way: every ribbon button, menu item and dialog runs a command from one
registry of 700+ commands, shared by the window, the command line, a JSON control channel and an
MCP server.

This branch lives in a fork of [rallegretti/PlanCraft](https://github.com/rallegretti/PlanCraft)
and is a separate code base from that project's main branch.

## What it does

- Gantt Chart, Tracking Gantt, Task Sheet, Task Usage, Network Diagram, Calendar, Timeline,
  Task/Resource Form, Resource Sheet/Usage/Graph and Team Planner views
- Critical path, slack, constraints, deadlines, baselines 0–10, resource leveling, working-time
  calendars, recurring tasks, subprojects, custom fields
- A ribbon like familiar office desktop apps (Task, Resource, Report, Project, View, Help and a
  view-specific Format tab), a File screen (New with templates, Open with recent and pinned
  plans, Info, Save As, Print with live preview and PDF, Share, Export) and an Options window
- Opens `.plan.json`, the XML interchange format and `.mpp` files from the desktop scheduling
  program's 2010-and-later releases (read only); exports PDF, HTML, SVG, CSV and Markdown
- Light, dark or follow-the-system theme

## Build

```sh
cargo build --release
./target/release/plancraft            # the window
./target/release/plancraft-cli --help # the command line
```

On Windows with more than one Visual Studio installed, you may need a `.cargo/config.toml`
pointing at the linker you use (it isn't committed because it is machine-specific).

## Drive it from scripts and agents

```sh
plancraft-cli run plan.plan.json --cmd task.insert --params '{"name":"Review","duration":"2d"}' --out plan.plan.json
plancraft-cli batch --actions weekly.json --in ./plans --out ./updated
plancraft-cli describe task.insert
plancraft-cli mcp            # an MCP server without a window
plancraft-cli mcp --bridge   # an MCP server that drives the open window
```

See `MECHANICS.md` for the scheduling rules and the `.mpp` file layout.
