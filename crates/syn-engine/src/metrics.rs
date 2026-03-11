use syn_types::WorkItem;

/// Print a summary of metrics for a completed work item.
pub fn print_summary(item: &WorkItem) {
    eprintln!("\n╔══════════════════════════════════════╗");
    eprintln!("║        Factory Run Complete           ║");
    eprintln!("╠══════════════════════════════════════╣");
    eprintln!("║  Work ID:        {:<20}║", item.id);
    eprintln!("║  Spec:           {:<20}║", item.spec_path.display());

    if let Some(secs) = item.metrics.cycle_time_secs {
        eprintln!("║  Cycle time:     {:<17.1}s ║", secs);
    }

    eprintln!("║  Total tokens:   {:<20}║", item.metrics.total_tokens);
    eprintln!("║  Rework count:   {:<20}║", item.metrics.rework_count);

    if let Some(fpy) = item.metrics.first_pass_yield {
        let label = if fpy { "YES" } else { "NO" };
        eprintln!("║  First-pass yield: {:<18}║", label);
    }

    eprintln!("║  Branch:         {:<20}║", item.branch);
    eprintln!("╚══════════════════════════════════════╝");
}
