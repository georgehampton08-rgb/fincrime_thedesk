using Godot;
using System;
using System.Text.Json;

public partial class Main : Control
{
    private SimBridge _simBridge;
    private Label _tickLabel;
    private Button _pauseButton;
    private Label _statusLabel;
    
    // KPIs
    private Label _kpiCustomers;
    private Label _kpiChurn;
    private Label _kpiComplaints;
    private Label _kpiSla;
    private Label _kpiNim;
    private Label _kpiEff;
    
    // Screens
    private Control _viewOverview;
    private Control _viewCustomers;
    private Control _viewComplaints;
    private Control _viewProducts;
    private Control _viewPnL;

    private Timer _gameTimer;
    private bool _isPlaying = false;

    public override void _Ready()
    {
        _simBridge = GetNode<SimBridge>("/root/SimBridge");
        
        _tickLabel = GetNodeOrNull<Label>("TopBar/TickLabel");
        
        // Buttons
        _pauseButton = GetNodeOrNull<Button>("TopBar/PauseButton");
        var btnOverview = GetNodeOrNull<Button>("TopBar/NavButtons/BtnOverview");
        var btnCustomers = GetNodeOrNull<Button>("TopBar/NavButtons/BtnCustomers");
        var btnComplaints = GetNodeOrNull<Button>("TopBar/NavButtons/BtnComplaints");
        var btnProducts = GetNodeOrNull<Button>("TopBar/NavButtons/BtnProducts");
        var btnPnL = GetNodeOrNull<Button>("TopBar/NavButtons/BtnPnL");
        
        _statusLabel = GetNodeOrNull<Label>("StatusPanel/Label");

        // KPI Panels
        _kpiCustomers = GetNodeOrNull<Label>("ContentArea/Overview/KPIGrid/Panel1/VBox/Value");
        _kpiChurn = GetNodeOrNull<Label>("ContentArea/Overview/KPIGrid/Panel2/VBox/Value");
        _kpiComplaints = GetNodeOrNull<Label>("ContentArea/Overview/KPIGrid/Panel3/VBox/Value");
        _kpiSla = GetNodeOrNull<Label>("ContentArea/Overview/KPIGrid/Panel4/VBox/Value");
        _kpiNim = GetNodeOrNull<Label>("ContentArea/Overview/KPIGrid/Panel5/VBox/Value");
        _kpiEff = GetNodeOrNull<Label>("ContentArea/Overview/KPIGrid/Panel6/VBox/Value");

        // Views
        _viewOverview = GetNodeOrNull<Control>("ContentArea/Overview");
        _viewCustomers = GetNodeOrNull<Control>("ContentArea/CustomersView");
        _viewComplaints = GetNodeOrNull<Control>("ContentArea/ComplaintsView");
        _viewProducts = GetNodeOrNull<Control>("ContentArea/ProductsView");
        _viewPnL = GetNodeOrNull<Control>("ContentArea/PnLReport");

        // Setup Timer
        _gameTimer = new Timer();
        _gameTimer.WaitTime = 0.5f; // 2 ticks per second
        _gameTimer.OneShot = false;
        _gameTimer.Timeout += OnGameTick;
        AddChild(_gameTimer);

        // Connect Signals
        if (_simBridge != null)
        {
            _simBridge.StateUpdated += OnStateUpdated;
            _simBridge.TickAdvanced += OnTickAdvanced;
            _simBridge.SimulationError += OnSimulationError;
            
            _simBridge.Start();
            _simBridge.RequestState();
        }

        if (_pauseButton != null) 
        {
            _pauseButton.Pressed += OnPausePressed;
            _pauseButton.Text = "PLAY"; // Start paused
        }
        
        if (btnOverview != null) btnOverview.Pressed += () => SwitchView(_viewOverview);
        if (btnCustomers != null) btnCustomers.Pressed += () => SwitchView(_viewCustomers);
        if (btnComplaints != null) btnComplaints.Pressed += () => SwitchView(_viewComplaints);
        if (btnProducts != null) btnProducts.Pressed += () => SwitchView(_viewProducts);
        if (btnPnL != null) btnPnL.Pressed += () => SwitchView(_viewPnL);
    }
    
    private void OnGameTick()
    {
        if (_simBridge != null)
        {
            _simBridge.SendTick(1); // Advance 1 tick
        }
    }

    private void SwitchView(Control view)
    {
        if (view == null) return;
        
        if (_viewOverview != null) _viewOverview.Visible = false;
        if (_viewCustomers != null) _viewCustomers.Visible = false;
        if (_viewComplaints != null) _viewComplaints.Visible = false;
        if (_viewProducts != null) _viewProducts.Visible = false;
        if (_viewPnL != null) _viewPnL.Visible = false;
        
        view.Visible = true;
    }

    private void OnTickAdvanced(ulong tick)
    {
        if (_tickLabel != null)
            _tickLabel.Text = $"Tick: {tick}";
    }

    private void OnStateUpdated(string json)
    {
        try 
        {
            using var doc = JsonDocument.Parse(json);
            var root = doc.RootElement;
            
            if (_kpiCustomers != null && root.TryGetProperty("active_customers", out var c))
                _kpiCustomers.Text = $"{c.GetInt64():N0}";

            if (_kpiChurn != null && root.TryGetProperty("churned_customers", out var ch))
                _kpiChurn.Text = $"{ch.GetInt64():N0}";

            if (_kpiComplaints != null && root.TryGetProperty("complaint_count", out var cc))
                _kpiComplaints.Text = $"{cc.GetInt64():N0}";

            if (_kpiSla != null && root.TryGetProperty("sla_breaches", out var sla))
                _kpiSla.Text = $"{sla.GetInt64():N0}";
                
            if (_kpiNim != null && root.TryGetProperty("nim", out var nim))
                _kpiNim.Text = $"{nim.GetDouble():F2}%";

            if (_kpiEff != null && root.TryGetProperty("efficiency_ratio", out var eff))
                _kpiEff.Text = $"{eff.GetDouble():F2}%";
                
            if (_statusLabel != null)
                _statusLabel.Text = "Connected";
                
            // Forward data to active views
            if (_viewComplaints != null && _viewComplaints.Visible)
            {
                 // Find the ComplaintsView script and update it
                 // This is a bit hacky, normally we'd allow the view to subscribe itself
                 // But for MVP we can just let SimBridge broadcast to everyone (which it does via Signal)
                 // The Views subscribe themselves in their _Ready()
            }
        }
        catch (Exception e)
        {
             GD.PrintErr($"UI Update Error: {e.Message}");
        }
    }

    private void OnSimulationError(string error)
    {
        GD.PrintErr($"Sim Error: {error}");
        if (_statusLabel != null)
            _statusLabel.Text = $"Error: {error}";
    }
    
    public void OnPausePressed()
    {
        _isPlaying = !_isPlaying;
        
        if (_isPlaying)
        {
            _gameTimer.Start();
            if (_pauseButton != null) _pauseButton.Text = "PAUSE";
        }
        else
        {
            _gameTimer.Stop();
            if (_pauseButton != null) _pauseButton.Text = "PLAY";
        }
    }
}
