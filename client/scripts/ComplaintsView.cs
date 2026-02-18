using Godot;
using System;
using System.Text.Json;
using System.Collections.Generic;

public partial class ComplaintsView : Control
{
    private SimBridge _simBridge;
    private ItemList _list;
    private Button _btnResolveRefund;
    private Button _btnResolveExplain;
    private Label _detailsLabel;
    
    // Store current complaint IDs mapping to list index
    private List<string> _complaintIds = new List<string>();

    public override void _Ready()
    {
        _simBridge = GetNode<SimBridge>("/root/SimBridge");
        
        _list = GetNodeOrNull<ItemList>("HBox/ListPanel/ComplaintList");
        _btnResolveRefund = GetNodeOrNull<Button>("HBox/DetailPanel/VBox/Actions/BtnRefund");
        _btnResolveExplain = GetNodeOrNull<Button>("HBox/DetailPanel/VBox/Actions/BtnExplain");
        _detailsLabel = GetNodeOrNull<Label>("HBox/DetailPanel/VBox/Details");

        if (_simBridge != null)
        {
            _simBridge.StateUpdated += OnStateUpdated;
        }

        if (_list != null) _list.ItemSelected += OnItemSelected;
        if (_btnResolveRefund != null) _btnResolveRefund.Pressed += () => ResolveSelected("refund", 100.0); // Example amount
        if (_btnResolveExplain != null) _btnResolveExplain.Pressed += () => ResolveSelected("explanation_only", 0.0);
    }

    private void OnStateUpdated(string json)
    {
        if (!Visible) return; // Opt: don't update if hidden
        
        try 
        {
            using var doc = JsonDocument.Parse(json);
            var root = doc.RootElement;
            
            if (root.TryGetProperty("complaints", out var complaintsArray))
            {
                UpdateList(complaintsArray);
            }
        }
        catch (Exception e)
        {
             GD.PrintErr($"Complaints Update Error: {e.Message}");
        }
    }

    private void UpdateList(JsonElement complaintsArray)
    {
        if (_list == null) return;
        
        // Simple full refresh for MVP
        // Save selection
        int selectedIdx = _list.GetSelectedItems().Length > 0 ? _list.GetSelectedItems()[0] : -1;
        string selectedId = (selectedIdx >= 0 && selectedIdx < _complaintIds.Count) ? _complaintIds[selectedIdx] : null;

        _list.Clear();
        _complaintIds.Clear();

        foreach (var c in complaintsArray.EnumerateArray())
        {
            string id = c.GetProperty("complaint_id").GetString();
            string issue = c.GetProperty("issue").GetString();
            string product = c.GetProperty("product").GetString();
            
            _list.AddItem($"{product}: {issue}");
            _complaintIds.Add(id);
        }
        
        // Restore selection if possible
        if (selectedId != null)
        {
            int newIdx = _complaintIds.IndexOf(selectedId);
            if (newIdx != -1)
            {
                 _list.Select(newIdx);
                 OnItemSelected(newIdx);
            }
        }
    }

    private void OnItemSelected(long index)
    {
        int idx = (int)index;
        if (idx < 0 || idx >= _complaintIds.Count) return;
        
        string id = _complaintIds[idx];
        if (_detailsLabel != null)
            _detailsLabel.Text = $"Selected Complaint: {id}\n\n(Details would go here...)";
            
        // Enable buttons
        if (_btnResolveRefund != null) _btnResolveRefund.Disabled = false;
        if (_btnResolveExplain != null) _btnResolveExplain.Disabled = false;
    }

    private void ResolveSelected(string resolution, double refund)
    {
        int selectedIdx = _list.GetSelectedItems().Length > 0 ? _list.GetSelectedItems()[0] : -1;
        if (selectedIdx < 0 || selectedIdx >= _complaintIds.Count) return;
        
        string id = _complaintIds[selectedIdx];
        
        var payload = new Godot.Collections.Dictionary
        {
            { "complaint_id", id },
            { "resolution", resolution },
            { "refund", refund }
        };
        
        _simBridge.SendCommand("resolve_complaint", payload);
        
        // Optimistic UI update or wait for next tick
        _detailsLabel.Text = "Resolving...";
    }
}
