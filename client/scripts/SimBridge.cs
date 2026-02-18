using Godot;
using System;
using System.Diagnostics;
using System.Text.Json;
using System.Text.Json.Nodes;
using System.Text.Json.Serialization;

public partial class SimBridge : Node
{
    private Process _simProcess;
    private bool _isRunning = false;

    [Signal] public delegate void StateUpdatedEventHandler(string jsonState);
    [Signal] public delegate void TickAdvancedEventHandler(ulong tick);
    [Signal] public delegate void SimulationErrorEventHandler(string error);

    public override void _Ready()
    {
        // Don't auto-start. Wait for Main to connect signals.
    }

    public void Start()
    {
        StartSimulation();
    }

    private void StartSimulation()
    {
        if (_isRunning) return;

            var exePath = ProjectSettings.GlobalizePath("res://sim-runner.exe");
            var workingDir = System.IO.Path.GetDirectoryName(exePath);
            
            GD.Print($"Launching sim-runner from: {exePath}");
            GD.Print($"Working Dir: {workingDir}");

            var startInfo = new ProcessStartInfo
            {
                FileName = exePath,
                WorkingDirectory = workingDir,
                Arguments = "--ticks 0 --ipc-mode",
                RedirectStandardInput = true,
                RedirectStandardOutput = true,
                RedirectStandardError = true,
                UseShellExecute = false,
                CreateNoWindow = true
            };

        try
        {
            _simProcess = new Process { StartInfo = startInfo };
            _simProcess.Start();
            _isRunning = true;
            ReadOutputAsync();
            ReadErrorAsync();
            GD.Print("SimRunner started successfully.");
        }
        catch (Exception e)
        {
            GD.PrintErr($"Failed to start sim-runner: {e.Message}");
            EmitSignal(SignalName.SimulationError, e.Message);
        }
    }

    private async void ReadErrorAsync()
    {
        while (_isRunning && !_simProcess.HasExited)
        {
             string line = await _simProcess.StandardError.ReadLineAsync();
             if (line == null) break;
             if (!string.IsNullOrWhiteSpace(line))
             {
                 GD.PrintErr($"SimRunner Stderr: {line}");
                 // Optional: Emit as error if critical, or just log
             }
        }
    }

    private async void ReadOutputAsync()
    {
        while (_isRunning && !_simProcess.HasExited)
        {
            string line = await _simProcess.StandardOutput.ReadLineAsync();
            if (line == null) break;

            if (string.IsNullOrWhiteSpace(line)) continue;
            
            // GD.Print($"RX: {line}"); // Uncomment to see raw JSON
            CallDeferred(nameof(ProcessMessage), line);
        }
        _isRunning = false;
        GD.Print("SimRunner process exited.");
    }

    private void ProcessMessage(string json)
    {
        try
        {
            // Simple validation: is it an error?
            using var doc = JsonDocument.Parse(json);
            var root = doc.RootElement;

            if (root.TryGetProperty("error", out var errorElement))
            {
                GD.PrintErr($"Sim Error: {errorElement}");
                EmitSignal(SignalName.SimulationError, errorElement.ToString());
                return;
            }

            EmitSignal(SignalName.StateUpdated, json);
            
            if (root.TryGetProperty("tick", out var tickElement))
            {
                if (tickElement.TryGetUInt64(out ulong tick))
                {
                    EmitSignal(SignalName.TickAdvanced, tick);
                }
            }
        }
        catch (Exception e)
        {
            GD.PrintErr($"Failed to parse sim output: {e.Message} | Raw: {json}");
        }
    }

    public void SendCommand(string cmd, Godot.Collections.Dictionary payload = null)
    {
        if (!_isRunning) return;

        var cmdObj = new JsonObject
        {
            ["type"] = "command",
            ["cmd"] = cmd,
            ["payload"] = payload != null ? JsonSerializer.SerializeToNode(payload) : new JsonObject()
        };
        
        WriteJson(cmdObj);
    }

    public void SendTick(ulong count)
    {
        if (!_isRunning) return;
        
        var cmdObj = new JsonObject
        {
            ["type"] = "tick",
            ["count"] = count
        };
        WriteJson(cmdObj);
    }
    
    public void RequestState()
    {
         if (!_isRunning) return;
         
         var cmdObj = new JsonObject
         {
             ["type"] = "get_state"
         };
         WriteJson(cmdObj);
    }

    private void WriteJson(JsonObject jsonObj)
    {
        if (!_isRunning) return;
        
        string json = jsonObj.ToJsonString(new JsonSerializerOptions { WriteIndented = false });
        // GD.Print($"TX: {json}");
        _simProcess.StandardInput.WriteLine(json);
        _simProcess.StandardInput.Flush();
    }

    public override void _ExitTree()
    {
         if (_isRunning)
        {
            try 
            {
                // Try clean quit first
                var quitCmd = new JsonObject { ["type"] = "quit" };
                WriteJson(quitCmd);
                _simProcess.WaitForExit(1000);
            }
            catch {}
            
            if (!_simProcess.HasExited)
            {
                _simProcess.Kill();
            }
        }
    }
}

