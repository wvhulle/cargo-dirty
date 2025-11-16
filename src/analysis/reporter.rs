use log::info;
use crate::parsing::RebuildReason;

pub fn print_rebuild_analysis(rebuild_reasons: &[RebuildReason]) {
    info!("🔍 REBUILD ANALYSIS SUMMARY");
    info!("═══════════════════════════════════════════════════════════════");
    info!("Found {} rebuild trigger(s):\n", rebuild_reasons.len());

    for (i, reason) in rebuild_reasons.iter().enumerate() {
        info!("{}. {}\n", i + 1, reason);
    }

    // Provide summary insights
    let env_changes = rebuild_reasons.iter().filter(|r| matches!(r, RebuildReason::EnvVarChanged { .. })).count();
    let dep_changes = rebuild_reasons.iter().filter(|r| matches!(r, RebuildReason::UnitDependencyInfoChanged { .. })).count();
    let target_changes = rebuild_reasons.iter().filter(|r| matches!(r, RebuildReason::TargetConfigurationChanged)).count();
    let file_changes = rebuild_reasons.iter().filter(|r| matches!(r, RebuildReason::FileChanged { .. })).count();

    print_summary_breakdown(env_changes, dep_changes, target_changes, file_changes);
    print_optimization_tips(env_changes, dep_changes, rebuild_reasons.len());

    info!("═══════════════════════════════════════════════════════════════");
}

fn print_summary_breakdown(env_changes: usize, dep_changes: usize, target_changes: usize, file_changes: usize) {
    info!("📊 SUMMARY BREAKDOWN:");
    if env_changes > 0 {
        info!("   • {env_changes} environment variable change(s) - Consider using consistent development environment");
    }
    if dep_changes > 0 {
        info!("   • {dep_changes} dependency rebuild(s) - Dependencies were modified or their fingerprints changed");
    }
    if target_changes > 0 {
        info!("   • {target_changes} target configuration change(s) - Build settings were modified");
    }
    if file_changes > 0 {
        info!("   • {file_changes} file change(s) - Source files or configuration were modified");
    }
}

fn print_optimization_tips(env_changes: usize, dep_changes: usize, total_changes: usize) {
    info!("\n💡 OPTIMIZATION TIPS:");
    if env_changes > dep_changes {
        info!("   • Most rebuilds are due to environment changes - use tools like direnv or nix-shell for consistent environments");
    }
    if dep_changes > 0 {
        info!("   • Use 'cargo build --keep-going' to continue building when some dependencies fail");
        info!("   • Consider workspace dependencies to reduce rebuild cascades");
    }
    if total_changes > 10 {
        info!("   • Many rebuild triggers detected - consider incremental development practices");
    }
}