use crate::parsing::RebuildReason;

pub fn print_rebuild_analysis(rebuild_reasons: &[RebuildReason]) {
    eprintln!("\nRebuild Analysis ({} triggers)\n", rebuild_reasons.len());

    for reason in rebuild_reasons {
        eprintln!("{reason}");
    }

    if rebuild_reasons.len() > 1 {
        let env_changes = rebuild_reasons
            .iter()
            .filter(|r| matches!(r, RebuildReason::EnvVarChanged { .. }))
            .count();
        let dep_changes = rebuild_reasons
            .iter()
            .filter(|r| matches!(r, RebuildReason::UnitDependencyInfoChanged { .. }))
            .count();
        let target_changes = rebuild_reasons
            .iter()
            .filter(|r| matches!(r, RebuildReason::TargetConfigurationChanged))
            .count();
        let file_changes = rebuild_reasons
            .iter()
            .filter(|r| matches!(r, RebuildReason::FileChanged { .. }))
            .count();

        print_summary_breakdown(env_changes, dep_changes, target_changes, file_changes);
        print_optimization_tips(env_changes, dep_changes, rebuild_reasons.len());
    }
}

fn print_summary_breakdown(
    env_changes: usize,
    dep_changes: usize,
    target_changes: usize,
    file_changes: usize,
) {
    let total = env_changes + dep_changes + target_changes + file_changes;
    if total == 0 {
        return;
    }

    eprintln!("\nSummary:");
    if env_changes > 0 {
        eprintln!(
            "   • {} env variable{}",
            env_changes,
            if env_changes > 1 { "s" } else { "" }
        );
    }
    if dep_changes > 0 {
        eprintln!(
            "   • {} dependenc{}",
            dep_changes,
            if dep_changes > 1 { "ies" } else { "y" }
        );
    }
    if target_changes > 0 {
        eprintln!(
            "   • {} config change{}",
            target_changes,
            if target_changes > 1 { "s" } else { "" }
        );
    }
    if file_changes > 0 {
        eprintln!(
            "   • {} file{}",
            file_changes,
            if file_changes > 1 { "s" } else { "" }
        );
    }
}

fn print_optimization_tips(env_changes: usize, dep_changes: usize, total_changes: usize) {
    let has_tips = env_changes > dep_changes || dep_changes > 0 || total_changes > 10;

    if !has_tips {
        return;
    }

    eprintln!("\nTips:");
    if env_changes > dep_changes && env_changes > 0 {
        eprintln!("   • Use direnv or nix-shell for consistent environments");
    }
    if dep_changes > 0 {
        eprintln!("   • Try 'cargo build --keep-going' for better CI performance");
        eprintln!("   • Consider workspace dependencies to reduce cascades");
    }
    if total_changes > 10 {
        eprintln!("   • Many triggers detected - consider incremental changes");
    }
}
