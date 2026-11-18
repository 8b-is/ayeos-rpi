# ayeos-rpi / nullclaw / agent / README.md — nullclaw agent design
# nullclaw — on-device assistance for Raspberry Pi 3/4 running ayeOS/linux.
#
# Reads skills from ~/.agents/skills/ (or /mnt/skills) and exposes a local MCP
# interface over the device UART (/dev/ttyAMA0) so a host can query the device
# without network. Skill frontmatter (name/description) is indexed at startup.
#
# Routes:
#   - skill list -> returns indexed SKILL.md names + descriptions
#   - skill run  -> executes a skill's local script against device state
#   - system     -> returns Pi model, kernel, throttle flags (vcgencmd)
#
# Wired to the g8t-plugins skill registry: `nullclaw` can be pointed at a cloned
# g8t-plugins repo and will load each plugin's SKILL.md as a local skill.
