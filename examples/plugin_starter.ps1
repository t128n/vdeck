# Minimal vDeck plugin starter in PowerShell (vdeck-plugin/1).
# Run with: pwsh -NoProfile -File examples/plugin_starter.ps1 --stdio

param(
    [switch]$stdio
)

if (-not $stdio -and ($args -notcontains "--stdio")) {
    [Console]::Error.WriteLine("run with --stdio")
    exit 2
}

$Protocol = "vdeck-plugin/1"
$Commands = @("ping", "toggle_state", "dynamic_deck")
$AppConfig = @{}
$PluginConfig = @{}

function Send-Msg([hashtable]$Msg) {
    $json = $Msg | ConvertTo-Json -Depth 10 -Compress
    [Console]::Out.WriteLine($json)
    [Console]::Out.Flush()
}

while ($null -ne ($raw = [Console]::In.ReadLine())) {
    $line = $raw.TrimStart([char]0xFEFF).Trim()
    if ([string]::IsNullOrWhiteSpace($line)) { continue }

    $msg = $line | ConvertFrom-Json
    switch ($msg.type) {
        "hello" {
            if ($msg.config) { $AppConfig = $msg.config }
            if ($msg.plugin_config) { $PluginConfig = $msg.plugin_config }
            Send-Msg @{
                type     = "hello"
                protocol = $Protocol
                name     = "example_ps1"
                commands = $Commands
            }
        }
        "call" {
            $id = [int64]$msg.id
            $cmd = [string]$msg.command
            $active = [bool]$msg.active

            switch ($cmd) {
                "ping" {
                    $greeting = if ($PluginConfig.greeting) { [string]$PluginConfig.greeting } else { "Pong from PowerShell" }
                    Send-Msg @{ type = "result"; id = $id; ok = $true; status = $greeting }
                }
                "toggle_state" {
                    $next = -not $active
                    Send-Msg @{
                        type   = "result"
                        id     = $id
                        ok     = $true
                        active = $next
                        status = $(if ($next) { "Active" } else { "Inactive" })
                    }
                }
                "dynamic_deck" {
                    Send-Msg @{
                        type      = "result"
                        id        = $id
                        ok        = $true
                        open_deck = "example_ps1_deck"
                        decks     = @(
                            @{
                                id     = "example_ps1_deck"
                                name   = "Example"
                                layers = @(
                                    @{
                                        name = "Items"
                                        keys = @(
                                            @{
                                                label  = "Ping"
                                                icon   = "circle"
                                                accent = "cyan"
                                                action = @{
                                                    type    = "plugin"
                                                    plugin  = "example_ps1"
                                                    command = "ping"
                                                }
                                            },
                                            @{
                                                label  = "Back"
                                                icon   = "back"
                                                accent = "orange"
                                                action = @{ type = "back" }
                                            }
                                        )
                                    }
                                )
                            }
                        )
                    }
                }
                default {
                    Send-Msg @{ type = "result"; id = $id; ok = $false; error = "unknown command: $cmd" }
                }
            }
        }
        "goodbye" {
            break
        }
    }
}
