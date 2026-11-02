#![forbid(unsafe_code)]

use clap::{Args, Parser, Subcommand};
use std::fs;
use std::path::PathBuf;
use std::process::ExitCode;

use parley::economics::compute_dialogue_economics;
use parley::envelope::{EnvelopeStatus, HarnessEnvelope};
use parley::grader::{evaluate_case_transcript, evaluate_suite};
use parley::ingest::ingest_harness_runs;
use parley::lockfile::{generate_lockfile, verify_lockfile, ParleyLockfile};
use parley::repo_audit::audit_repository;
use parley::report::{
    export_otel_spans, export_reval_ratchet, generate_mermaid_sequence, generate_mermaid_xychart,
};
use parley::simulator::{simulate_conversation, SystemProfile};
use parley::spec::SuiteSpec;
use parley::transcript::load_transcript_from_jsonl;

const SOFTFACTORY_PACK_STR: &str = include_str!("../packs/softfactory.toml");

#[derive(Parser, Debug)]
#[command(
    name = "parley",
    author = "TechTide Ohio",
    version = "0.1.0",
    about = "TideOS Eval Kit (Tool 8): Simulated users for multi-turn conversational evaluation",
    long_about = None
)]
pub struct Cli {
    #[arg(long, global = true, help = "Emit canonical HarnessEnvelope<T> JSON")]
    pub json: bool,

    #[arg(
        long,
        global = true,
        default_value_t = true,
        help = "Ring terminal audio alert (\\x07) on policy violation, sycophancy, or state amnesia"
    )]
    pub bell: bool,

    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    #[command(about = "Initialize a declarative multi-turn evaluation suite")]
    Init(InitArgs),

    #[command(about = "Display the Single-Turn Apparent Pass vs Multi-Turn State Retention Table")]
    Table(TableArgs),

    #[command(
        about = "Simulate a multi-turn conversation with deterministic persona and system profiles"
    )]
    Simulate(SimulateArgs),

    #[command(about = "Score a recorded multi-turn transcript JSONL file")]
    Score(ScoreArgs),

    #[command(
        about = "Evaluate and gate a multi-case suite against multi-turn memory and policy thresholds"
    )]
    Check(CheckArgs),

    #[command(about = "Compare two multi-turn transcripts head-to-head")]
    Compare(CompareArgs),

    #[command(about = "Ingest multi-agent harness execution runs from agent-harness/runs")]
    Ingest(IngestArgs),

    #[command(about = "Generate a sha256-framed-v1 cryptographic lockfile")]
    Lock(LockArgs),

    #[command(about = "Verify suite evidence and digests against a cryptographic lockfile")]
    Verify(VerifyArgs),

    #[command(about = "Audit a repository for Soft Factory hygiene and zero em-dash compliance")]
    Repo(RepoArgs),

    #[command(
        about = "Export multi-turn metrics as Mermaid sequence, OTel spans, or reval ratchet"
    )]
    Export(ExportArgs),
}

#[derive(Args, Debug)]
pub struct InitArgs {
    #[arg(long, default_value = "softfactory", help = "Preset template pack")]
    pub pack: String,

    #[arg(
        long,
        default_value = "parley.toml",
        help = "Output path for suite file"
    )]
    pub out: PathBuf,
}

#[derive(Args, Debug)]
pub struct TableArgs {
    #[arg(
        long,
        default_value_t = 0.12,
        help = "Per-turn state memory decay rate"
    )]
    pub decay_rate: f64,

    #[arg(
        long,
        default_value_t = 0.25,
        help = "Adversarial pushback collapse probability"
    )]
    pub pushback_collapse: f64,
}

#[derive(Args, Debug)]
pub struct SimulateArgs {
    #[arg(
        long,
        default_value = "packs/softfactory.toml",
        help = "Path to suite TOML"
    )]
    pub suite: PathBuf,

    #[arg(
        long,
        default_value = "sf-vsp-rls-clarification",
        help = "Case identifier to simulate"
    )]
    pub case: String,

    #[arg(
        long,
        default_value = "optimal-clarifier",
        help = "System profile: optimal-clarifier | premature-guesser | amnesiac-drifter | sycophantic-caver"
    )]
    pub profile: String,

    #[arg(long, default_value_t = 42, help = "Deterministic PRNG seed")]
    pub seed: u64,

    #[arg(long, help = "Optional path to write simulated JSONL transcript into")]
    pub out: Option<PathBuf>,
}

#[derive(Args, Debug)]
pub struct ScoreArgs {
    #[arg(
        long,
        default_value = "packs/softfactory.toml",
        help = "Path to suite TOML"
    )]
    pub suite: PathBuf,

    #[arg(
        long,
        default_value = "sf-vsp-rls-clarification",
        help = "Case identifier to score"
    )]
    pub case: String,

    #[arg(long, help = "Path to multi-turn JSONL transcript")]
    pub transcript: PathBuf,
}

#[derive(Args, Debug)]
pub struct CheckArgs {
    #[arg(
        long,
        default_value = "packs/softfactory.toml",
        help = "Path to suite TOML file"
    )]
    pub suite: PathBuf,

    #[arg(long, default_value = ".", help = "Root directory for relative paths")]
    pub root: PathBuf,

    #[arg(long, help = "Optional path to cryptographic lockfile to verify")]
    pub lock: Option<PathBuf>,
}

#[derive(Args, Debug)]
pub struct CompareArgs {
    #[arg(
        long,
        default_value = "packs/softfactory.toml",
        help = "Path to suite TOML"
    )]
    pub suite: PathBuf,

    #[arg(long, help = "Case identifier")]
    pub case: String,

    #[arg(long, help = "Baseline transcript JSONL")]
    pub baseline: PathBuf,

    #[arg(long, help = "Candidate transcript JSONL")]
    pub candidate: PathBuf,
}

#[derive(Args, Debug)]
pub struct IngestArgs {
    #[arg(
        long,
        default_value = "Tools/agent-harness/runs",
        help = "Path to runs directory"
    )]
    pub runs_dir: PathBuf,
}

#[derive(Args, Debug)]
pub struct LockArgs {
    #[arg(
        long,
        default_value = "packs/softfactory.toml",
        help = "Path to suite TOML file"
    )]
    pub suite: PathBuf,

    #[arg(long, default_value = ".", help = "Root directory for relative paths")]
    pub root: PathBuf,

    #[arg(
        long,
        default_value = "packs/softfactory.lock.json",
        help = "Output path for lockfile"
    )]
    pub out: PathBuf,
}

#[derive(Args, Debug)]
pub struct VerifyArgs {
    #[arg(
        long,
        default_value = "packs/softfactory.toml",
        help = "Path to suite TOML file"
    )]
    pub suite: PathBuf,

    #[arg(long, default_value = ".", help = "Root directory for relative paths")]
    pub root: PathBuf,

    #[arg(
        long,
        default_value = "packs/softfactory.lock.json",
        help = "Path to lockfile to verify"
    )]
    pub lock: PathBuf,
}

#[derive(Args, Debug)]
pub struct RepoArgs {
    #[arg(default_value = ".", help = "Path to repository to audit")]
    pub path: PathBuf,
}

#[derive(Args, Debug)]
pub struct ExportArgs {
    #[arg(
        long,
        default_value = "packs/softfactory.toml",
        help = "Path to suite TOML file"
    )]
    pub suite: PathBuf,

    #[arg(long, default_value = ".", help = "Root directory for relative paths")]
    pub root: PathBuf,

    #[arg(
        long,
        default_value = "mermaid",
        help = "Export format: mermaid | otel | reval | xychart"
    )]
    pub format: String,

    #[arg(long, help = "Optional output file path")]
    pub out: Option<PathBuf>,
}

fn ring_bell() {
    print!("\x07");
}

fn main() -> Result<ExitCode, anyhow::Error> {
    let cli = Cli::parse();

    match &cli.command {
        Commands::Init(args) => handle_init(args, cli.json),
        Commands::Table(args) => handle_table(args, cli.json),
        Commands::Simulate(args) => handle_simulate(args, cli.json, cli.bell),
        Commands::Score(args) => handle_score(args, cli.json, cli.bell),
        Commands::Check(args) => handle_check(args, cli.json, cli.bell),
        Commands::Compare(args) => handle_compare(args, cli.json, cli.bell),
        Commands::Ingest(args) => handle_ingest(args, cli.json),
        Commands::Lock(args) => handle_lock(args, cli.json),
        Commands::Verify(args) => handle_verify(args, cli.json, cli.bell),
        Commands::Repo(args) => handle_repo(args, cli.json, cli.bell),
        Commands::Export(args) => handle_export(args, cli.json),
    }
}

fn handle_init(args: &InitArgs, json: bool) -> Result<ExitCode, anyhow::Error> {
    fs::write(&args.out, SOFTFACTORY_PACK_STR)?;
    if json {
        let env = HarnessEnvelope::success(
            format!("Initialized suite at {}", args.out.display()),
            serde_json::json!({ "path": args.out.display().to_string(), "pack": args.pack }),
        )
        .with_artifact(args.out.display().to_string())
        .with_next_action(format!("parley check --suite {}", args.out.display()));
        env.print_json()?;
    } else {
        println!("Initialized suite specification at {}", args.out.display());
    }
    Ok(ExitCode::SUCCESS)
}

fn handle_table(args: &TableArgs, json: bool) -> Result<ExitCode, anyhow::Error> {
    let turns = [1, 2, 3, 4, 5, 6, 8, 10];
    let mut rows = Vec::new();

    for &t in &turns {
        let single_turn_apparent = 95.0;
        let mut multi_turn_survival = 0.95 * (1.0 - args.decay_rate).powi(t - 1);
        if t >= 3 {
            multi_turn_survival *= 1.0 - args.pushback_collapse;
        }
        let gap = single_turn_apparent - (multi_turn_survival * 100.0);
        rows.push(serde_json::json!({
            "turn": t,
            "single_turn_apparent_pct": single_turn_apparent,
            "multi_turn_survival_pct": multi_turn_survival * 100.0,
            "state_decay_gap_pct": gap,
        }));
    }

    if json {
        let env = HarnessEnvelope::success(
            "Single-Turn Apparent Pass vs Multi-Turn State Retention Table",
            serde_json::json!({ "rows": rows }),
        );
        env.print_json()?;
    } else {
        println!("=========================================================================");
        println!(" Single-Turn Apparent Pass vs Multi-Turn State Retention & Policy Survival");
        println!(
            " Decay rate per turn: {:.1}%, Pushback collapse: {:.1}%",
            args.decay_rate * 100.0,
            args.pushback_collapse * 100.0
        );
        println!("=========================================================================");
        println!(
            "{:<6} | {:<20} | {:<20} | {:<16}",
            "Turn", "Single-Turn (Demo)", "Multi-Turn (Prod)", "State Decay Gap"
        );
        println!("-------+----------------------+----------------------+-----------------");
        for r in &rows {
            println!(
                "{:<6} | {:>18.1}% | {:>18.1}% | {:>14.1}%",
                r["turn"],
                r["single_turn_apparent_pct"].as_f64().unwrap_or(0.0),
                r["multi_turn_survival_pct"].as_f64().unwrap_or(0.0),
                r["state_decay_gap_pct"].as_f64().unwrap_or(0.0)
            );
        }
        println!("=========================================================================");
        println!("Notice: Single-turn evals stay at ~95% pass rate while multi-turn state");
        println!("retention collapses by Turn 4 once pushbacks and scope shifts compound!");
    }

    Ok(ExitCode::SUCCESS)
}

fn handle_simulate(args: &SimulateArgs, json: bool, bell: bool) -> Result<ExitCode, anyhow::Error> {
    let suite = SuiteSpec::from_file(&args.suite)?;
    let case = suite
        .cases
        .iter()
        .find(|c| c.id == args.case)
        .ok_or_else(|| anyhow::anyhow!("case '{}' not found in suite", args.case))?;

    let profile = SystemProfile::parse(&args.profile)
        .ok_or_else(|| anyhow::anyhow!("unknown profile '{}'", args.profile))?;

    let outcome = simulate_conversation(case, profile, args.seed)?;

    if let Some(ref out_p) = args.out {
        let mut buf = String::new();
        for t in &outcome.turns {
            buf.push_str(&serde_json::to_string(t)?);
            buf.push('\n');
        }
        fs::write(out_p, buf)?;
    }

    let scorecard = evaluate_case_transcript(case, &outcome.turns);

    if !scorecard.passed && bell {
        ring_bell();
    }

    if json {
        let env = HarnessEnvelope::success(
            format!(
                "Simulated {} turns for case {}: regime={:?}, passed={}",
                outcome.turns.len(),
                case.id,
                scorecard.memory.regime,
                scorecard.passed
            ),
            serde_json::json!({
                "case_id": case.id,
                "profile": args.profile,
                "scorecard": scorecard,
                "termination_reason": outcome.termination_reason,
            }),
        );
        env.print_json()?;
    } else {
        println!("=========================================================================");
        println!(
            " Multi-Turn Simulation: {} (Profile: {})",
            case.id, args.profile
        );
        println!("=========================================================================");
        println!(" Total Turns:          {}", outcome.turns.len());
        println!(" Termination Reason:   {}", outcome.termination_reason);
        println!(
            " Dialogue Regime:      {:?} ({})",
            scorecard.memory.regime,
            scorecard.memory.regime.description()
        );
        println!(" Goal Completed:       {}", scorecard.goal_completed);
        println!(
            " Goal Score:           {:.2}%",
            scorecard.goal_score * 100.0
        );
        println!(" Turn Efficiency:      {:.2}", scorecard.turn_efficiency);
        println!(
            " Memory Retention:     {:.2}%",
            scorecard.memory.memory_retention_rate * 100.0
        );
        println!(
            " Sycophancy Resistance:{:.2}%",
            scorecard.memory.sycophancy_resistance_rate * 100.0
        );
        println!(
            " Status:               {}",
            if scorecard.passed { "PASSED" } else { "FAILED" }
        );
        println!("=========================================================================");
    }

    if scorecard.passed {
        Ok(ExitCode::SUCCESS)
    } else {
        Ok(ExitCode::from(1))
    }
}

fn handle_score(args: &ScoreArgs, json: bool, bell: bool) -> Result<ExitCode, anyhow::Error> {
    let suite = SuiteSpec::from_file(&args.suite)?;
    let case = suite
        .cases
        .iter()
        .find(|c| c.id == args.case)
        .ok_or_else(|| anyhow::anyhow!("case '{}' not found in suite", args.case))?;

    let turns = load_transcript_from_jsonl(&args.transcript)?;
    let scorecard = evaluate_case_transcript(case, &turns);
    let economics = compute_dialogue_economics(&turns);

    if !scorecard.passed && bell {
        ring_bell();
    }

    if json {
        let env = HarnessEnvelope::success(
            format!(
                "Scored transcript for case {}: passed={}, goal_score={:.2}%, regime={:?}",
                case.id,
                scorecard.passed,
                scorecard.goal_score * 100.0,
                scorecard.memory.regime
            ),
            serde_json::json!({
                "case_id": case.id,
                "scorecard": scorecard,
                "economics": economics,
            }),
        );
        env.print_json()?;
    } else {
        println!("=========================================================================");
        println!(" Multi-Turn Transcript Evaluation: {}", case.id);
        println!("=========================================================================");
        println!(" Transcript File:      {}", args.transcript.display());
        println!(" Dialogue Turns:       {}", scorecard.total_turns);
        println!(" Dialogue Regime:      {:?}", scorecard.memory.regime);
        println!(" Goal Completed:       {}", scorecard.goal_completed);
        println!(
            " Goal Score:           {:.2}%",
            scorecard.goal_score * 100.0
        );
        println!(" Turn Efficiency:      {:.2}", scorecard.turn_efficiency);
        println!(
            " Clarification Recall: {:.2}%",
            scorecard.clarification_recall * 100.0
        );
        println!(
            " Memory Retention:     {:.2}%",
            scorecard.memory.memory_retention_rate * 100.0
        );
        println!(
            " Sycophancy Resistance:{:.2}%",
            scorecard.memory.sycophancy_resistance_rate * 100.0
        );
        println!(
            " Policy Violations:    {}",
            scorecard.memory.policy_violations.len()
        );
        println!(
            " Context Penalty:      {:.2}x",
            economics.context_accumulation_penalty_ratio
        );
        println!(
            " Status:               {}",
            if scorecard.passed { "PASSED" } else { "FAILED" }
        );
        println!("=========================================================================");
    }

    if scorecard.passed {
        Ok(ExitCode::SUCCESS)
    } else {
        Ok(ExitCode::from(1))
    }
}

fn handle_check(args: &CheckArgs, json: bool, bell: bool) -> Result<ExitCode, anyhow::Error> {
    let suite = SuiteSpec::from_file(&args.suite)?;
    suite.validate()?;

    if let Some(ref lock_p) = args.lock {
        let lock_content = fs::read_to_string(lock_p)?;
        let lock: ParleyLockfile = serde_json::from_str(&lock_content)?;
        if let Err(e) = verify_lockfile(&suite, &lock, &args.root) {
            if bell {
                ring_bell();
            }
            if json {
                let env = HarnessEnvelope::error(
                    format!("Lockfile drift detected: {e}"),
                    serde_json::json!({ "error": e.to_string() }),
                );
                env.print_json()?;
            } else {
                eprintln!("Error: lockfile drift detected: {e}");
            }
            return Ok(ExitCode::from(2));
        }
    }

    let mut scorecards = Vec::new();
    for case in &suite.cases {
        let transcript_abs = args.root.join(&case.transcript_path);
        let turns = load_transcript_from_jsonl(&transcript_abs)?;
        let sc = evaluate_case_transcript(case, &turns);
        scorecards.push(sc);
    }

    let summary = evaluate_suite(&suite, &scorecards);

    if !summary.all_passed && bell {
        ring_bell();
    }

    if json {
        let status = if summary.all_passed {
            EnvelopeStatus::Success
        } else {
            EnvelopeStatus::Error
        };
        let env = HarnessEnvelope {
            status,
            summary: format!(
                "Suite {}: {}/{} cases passed, avg goal={:.2}%, efficiency={:.2}, memory={:.2}%",
                summary.suite_id,
                summary.passed_cases,
                summary.total_cases,
                summary.macro_avg_goal_score * 100.0,
                summary.macro_avg_turn_efficiency,
                summary.macro_avg_memory_retention * 100.0
            ),
            next_actions: if summary.all_passed {
                vec!["All multi-turn state, memory, and policy gates passed".to_string()]
            } else {
                vec!["Review blocker failures and state amnesia in failing cases".to_string()]
            },
            artifacts: vec![args.suite.display().to_string()],
            data: serde_json::json!({
                "summary": summary,
                "cases": scorecards,
            }),
        };
        env.print_json()?;
    } else {
        println!("=========================================================================");
        println!(" TideOS Multi-Turn Evaluation Gate: {}", summary.suite_id);
        println!("=========================================================================");
        println!(
            " Passed Cases:         {}/{}",
            summary.passed_cases, summary.total_cases
        );
        println!(" Blocker Failures:     {}", summary.blocker_failures);
        println!(
            " Macro Avg Goal Score: {:.2}%",
            summary.macro_avg_goal_score * 100.0
        );
        println!(
            " Macro Avg Efficiency: {:.2}",
            summary.macro_avg_turn_efficiency
        );
        println!(
            " Macro Avg Memory:     {:.2}%",
            summary.macro_avg_memory_retention * 100.0
        );
        println!(
            " Macro Avg Sycophancy: {:.2}%",
            summary.macro_avg_sycophancy_resistance * 100.0
        );
        println!("-------------------------------------------------------------------------");
        println!(
            "{:<28} | {:<8} | {:<8} | {:<10} | {:<18}",
            "Case", "Stratum", "Turns", "Memory", "Regime"
        );
        println!(
            "-----------------------------+----------+----------+------------+-------------------"
        );
        for sc in &scorecards {
            println!(
                "{:<28} | {:<8} | {:<8} | {:>9.1}% | {:<18}",
                sc.case_id,
                sc.stratum,
                sc.total_turns,
                sc.memory.memory_retention_rate * 100.0,
                format!("{:?}", sc.memory.regime)
            );
        }
        println!("=========================================================================");
        if summary.all_passed {
            println!("GATE PASSED: All multi-turn conversation and policy requirements verified.");
        } else {
            println!("GATE FAILED: Blocker failures or threshold deficits detected.");
        }
    }

    if summary.all_passed {
        Ok(ExitCode::SUCCESS)
    } else {
        Ok(ExitCode::from(1))
    }
}

fn handle_compare(args: &CompareArgs, json: bool, bell: bool) -> Result<ExitCode, anyhow::Error> {
    let suite = SuiteSpec::from_file(&args.suite)?;
    let case = suite
        .cases
        .iter()
        .find(|c| c.id == args.case)
        .ok_or_else(|| anyhow::anyhow!("case '{}' not found in suite", args.case))?;

    let turns_base = load_transcript_from_jsonl(&args.baseline)?;
    let turns_cand = load_transcript_from_jsonl(&args.candidate)?;

    let sc_base = evaluate_case_transcript(case, &turns_base);
    let sc_cand = evaluate_case_transcript(case, &turns_cand);

    if (!sc_cand.passed && sc_base.passed) && bell {
        ring_bell();
    }

    if json {
        let env = HarnessEnvelope::success(
            format!("Compared baseline vs candidate for case {}", case.id),
            serde_json::json!({
                "case_id": case.id,
                "baseline": sc_base,
                "candidate": sc_cand,
            }),
        );
        env.print_json()?;
    } else {
        println!("=========================================================================");
        println!(" Head-to-Head Multi-Turn Comparison: {}", case.id);
        println!("=========================================================================");
        println!(
            "{:<24} | {:<16} | {:<16}",
            "Metric", "Baseline", "Candidate"
        );
        println!("-------------------------+------------------+-----------------");
        println!(
            "{:<24} | {:<16} | {:<16}",
            "Status",
            if sc_base.passed { "PASSED" } else { "FAILED" },
            if sc_cand.passed { "PASSED" } else { "FAILED" }
        );
        println!(
            "{:<24} | {:<16} | {:<16}",
            "Goal Completed", sc_base.goal_completed, sc_cand.goal_completed
        );
        println!(
            "{:<24} | {:>15.1}% | {:>15.1}%",
            "Goal Score",
            sc_base.goal_score * 100.0,
            sc_cand.goal_score * 100.0
        );
        println!(
            "{:<24} | {:<16} | {:<16}",
            "Total Turns", sc_base.total_turns, sc_cand.total_turns
        );
        println!(
            "{:<24} | {:>16.2} | {:>16.2}",
            "Turn Efficiency", sc_base.turn_efficiency, sc_cand.turn_efficiency
        );
        println!(
            "{:<24} | {:>15.1}% | {:>15.1}%",
            "Memory Retention",
            sc_base.memory.memory_retention_rate * 100.0,
            sc_cand.memory.memory_retention_rate * 100.0
        );
        println!(
            "{:<24} | {:>15.1}% | {:>15.1}%",
            "Sycophancy Resist",
            sc_base.memory.sycophancy_resistance_rate * 100.0,
            sc_cand.memory.sycophancy_resistance_rate * 100.0
        );
        println!(
            "{:<24} | {:<16} | {:<16}",
            "Regime",
            format!("{:?}", sc_base.memory.regime),
            format!("{:?}", sc_cand.memory.regime)
        );
        println!("=========================================================================");
    }

    Ok(ExitCode::SUCCESS)
}

fn handle_ingest(args: &IngestArgs, json: bool) -> Result<ExitCode, anyhow::Error> {
    let convs = ingest_harness_runs(&args.runs_dir)?;
    if json {
        let env = HarnessEnvelope::success(
            format!(
                "Ingested {} conversations from {}",
                convs.len(),
                args.runs_dir.display()
            ),
            serde_json::json!({
                "count": convs.len(),
                "sessions": convs.iter().map(|c| &c.session_id).collect::<Vec<_>>()
            }),
        );
        env.print_json()?;
    } else {
        println!(
            "Ingested {} conversations from {}",
            convs.len(),
            args.runs_dir.display()
        );
        for c in &convs {
            println!(" - {}: {} turns", c.session_id, c.turns.len());
        }
    }
    Ok(ExitCode::SUCCESS)
}

fn handle_lock(args: &LockArgs, json: bool) -> Result<ExitCode, anyhow::Error> {
    let suite = SuiteSpec::from_file(&args.suite)?;
    let lock = generate_lockfile(&suite, &args.root)?;
    let serialized = serde_json::to_string_pretty(&lock)?;
    fs::write(&args.out, serialized)?;

    if json {
        let env = HarnessEnvelope::success(
            format!("Generated lockfile at {}", args.out.display()),
            serde_json::json!({ "path": args.out.display().to_string(), "digest": lock.overall_lock_digest }),
        )
        .with_artifact(args.out.display().to_string());
        env.print_json()?;
    } else {
        println!("Generated lockfile at {}", args.out.display());
        println!("Overall lock digest: {}", lock.overall_lock_digest);
    }
    Ok(ExitCode::SUCCESS)
}

fn handle_verify(args: &VerifyArgs, json: bool, bell: bool) -> Result<ExitCode, anyhow::Error> {
    let suite = SuiteSpec::from_file(&args.suite)?;
    let lock_content = fs::read_to_string(&args.lock)?;
    let lock: ParleyLockfile = serde_json::from_str(&lock_content)?;

    match verify_lockfile(&suite, &lock, &args.root) {
        Ok(()) => {
            if json {
                let env = HarnessEnvelope::success(
                    format!("Lockfile verification passed for {}", args.lock.display()),
                    serde_json::json!({ "status": "verified" }),
                );
                env.print_json()?;
            } else {
                println!(
                    "Lockfile verification passed: zero drift in suite and transcript digests."
                );
            }
            Ok(ExitCode::SUCCESS)
        }
        Err(e) => {
            if bell {
                ring_bell();
            }
            if json {
                let env = HarnessEnvelope::error(
                    format!("Lockfile verification failed: {e}"),
                    serde_json::json!({ "error": e.to_string() }),
                );
                env.print_json()?;
            } else {
                eprintln!("Lockfile verification failed: {e}");
            }
            Ok(ExitCode::from(1))
        }
    }
}

fn handle_repo(args: &RepoArgs, json: bool, bell: bool) -> Result<ExitCode, anyhow::Error> {
    let report = audit_repository(&args.path);
    if !report.passed && bell {
        ring_bell();
    }

    if json {
        let status = if report.passed {
            EnvelopeStatus::Success
        } else {
            EnvelopeStatus::Error
        };
        let env = HarnessEnvelope {
            status,
            summary: format!(
                "Repository audit: {}/{} checks passed",
                report.passed_checks, report.total_checks
            ),
            next_actions: if report.passed {
                vec!["All repository hygiene checks passed".to_string()]
            } else {
                vec!["Resolve repository audit failures before publishing".to_string()]
            },
            artifacts: vec![args.path.display().to_string()],
            data: serde_json::json!(report),
        };
        env.print_json()?;
    } else {
        println!("=========================================================================");
        println!(" Repository Hygiene Audit: {}", report.target_path);
        println!("=========================================================================");
        println!(
            " Passed Checks: {}/{}",
            report.passed_checks, report.total_checks
        );
        println!("-------------------------------------------------------------------------");
        for c in &report.checks {
            let status = if c.passed { "PASS" } else { "FAIL" };
            println!("{:<30} | {:<6} | {}", c.name, status, c.detail);
        }
        println!("=========================================================================");
    }

    if report.passed {
        Ok(ExitCode::SUCCESS)
    } else {
        Ok(ExitCode::from(1))
    }
}

fn handle_export(args: &ExportArgs, json: bool) -> Result<ExitCode, anyhow::Error> {
    let suite = SuiteSpec::from_file(&args.suite)?;
    let mut scorecards = Vec::new();
    let mut first_turns = Vec::new();

    for (idx, case) in suite.cases.iter().enumerate() {
        let transcript_abs = args.root.join(&case.transcript_path);
        if let Ok(turns) = load_transcript_from_jsonl(&transcript_abs) {
            let sc = evaluate_case_transcript(case, &turns);
            if idx == 0 {
                first_turns = turns;
            }
            scorecards.push(sc);
        }
    }

    let summary = evaluate_suite(&suite, &scorecards);

    let output_str = match args.format.to_lowercase().as_str() {
        "mermaid" => {
            if let Some(sc) = scorecards.first() {
                generate_mermaid_sequence(sc, &first_turns)
            } else {
                "sequenceDiagram\n    Note: No cases available\n".to_string()
            }
        }
        "xychart" => generate_mermaid_xychart(0.12, 0.25),
        "otel" => serde_json::to_string_pretty(&export_otel_spans(&summary, &scorecards))?,
        "reval" => serde_json::to_string_pretty(&export_reval_ratchet(&summary))?,
        other => anyhow::bail!("unknown export format '{}'", other),
    };

    if let Some(ref out_p) = args.out {
        fs::write(out_p, &output_str)?;
    }

    if json {
        let env = HarnessEnvelope::success(
            format!("Exported {} format", args.format),
            serde_json::json!({ "format": args.format, "output": output_str }),
        );
        env.print_json()?;
    } else {
        println!("{output_str}");
    }

    Ok(ExitCode::SUCCESS)
}
