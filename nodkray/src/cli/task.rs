//! `nodkray task` — ODD/SDD preparation (fases 3-5). ST execution stays in fase 2.

use clap::{Args, Subcommand};
use serde::Serialize;
use ulid::Ulid;

use crate::cli::Context;
use crate::core::workflow::{
    prepare_sdd, select_review_depth, write_task_md, OddTask, ReviewDepth, WorkflowKind,
};
use crate::error::{NodkrayError, NodkrayResult};

#[derive(Debug, Args)]
pub struct TaskArgs {
    #[command(subcommand)]
    pub command: TaskCommand,
}

#[derive(Debug, Subcommand)]
pub enum TaskCommand {
    /// Create/run a task. ODD writes task.md; SDD prepares Spec-Kit stages.
    Run(TaskRunArgs),
    Inspect {
        id: String,
    },
    Cancel {
        id: String,
    },
}

#[derive(Debug, Args)]
pub struct TaskRunArgs {
    /// Task description.
    pub description: String,
    /// Force workflow: st, odd, sdd, auto.
    #[arg(long)]
    pub workflow: Option<String>,
    /// Force review depth: fast, balanced, deep.
    #[arg(long)]
    pub review: Option<String>,
    /// Create a constitution if SDD needs one and none exists.
    #[arg(long)]
    pub create_constitution: bool,
    /// Reduce worker interaction (does not change the workflow).
    #[arg(long)]
    pub yolo: bool,
}

#[derive(Debug, Serialize)]
struct TaskRunReport {
    task_id: String,
    workflow: String,
    review_depth: String,
    yolo: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    artefact: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    sdd: Option<crate::core::workflow::sdd::SddPlan>,
    note: String,
}

pub fn run(ctx: &Context, args: TaskArgs) -> NodkrayResult<i32> {
    match args.command {
        TaskCommand::Run(run_args) => run_task(ctx, run_args),
        TaskCommand::Inspect { id } => {
            let project = ctx.project_context()?;
            let text = crate::core::workflow::odd::read_task_md(&project.root, &id)?;
            ctx.output.emit_text(text);
            ctx.output.emit_json(&serde_json::json!({ "id": id }));
            Ok(0)
        }
        TaskCommand::Cancel { id } => {
            ctx.output.emit_text(format!("cancel requested for {id} (no running worker in this branch)"));
            ctx.output.emit_json(&serde_json::json!({ "id": id, "status": "cancelled" }));
            Ok(0)
        }
    }
}

fn run_task(ctx: &Context, args: TaskRunArgs) -> NodkrayResult<i32> {
    let project = ctx.project_context()?;
    let workflow = parse_workflow(args.workflow.as_deref())?;
    let override_depth = match args.review.as_deref() {
        None => None,
        Some(value) => Some(ReviewDepth::parse(value).ok_or_else(|| {
            NodkrayError::user_input("INVALID_REVIEW_DEPTH", format!("unknown review depth '{value}'"))
        })?),
    };
    let effort = heuristic_effort(&args.description);
    let depth = select_review_depth(
        workflow,
        effort,
        &project.config.review.thresholds,
        override_depth,
    );
    let task_id = format!("task_{}", Ulid::new());

    let mut report = TaskRunReport {
        task_id: task_id.clone(),
        workflow: workflow.as_str().to_string(),
        review_depth: depth.as_str().to_string(),
        yolo: args.yolo,
        artefact: None,
        sdd: None,
        note: String::new(),
    };

    match workflow {
        WorkflowKind::Odd => {
            let path = write_task_md(
                &project.root,
                &task_id,
                &OddTask::from_description(&args.description),
            )?;
            report.artefact = Some(path.display().to_string());
            report.note = "ODD artefact written; worker execution is owned by fase 2".into();
        }
        WorkflowKind::Sdd => {
            report.sdd = Some(prepare_sdd(
                &project.root,
                args.create_constitution,
                &[],
            )?);
            report.note = "SDD plan prepared via SpecKitAdapter; stages are not executed here".into();
        }
        WorkflowKind::St => {
            report.note = "ST execution is implemented in fases 0-2; this branch only records the decision".into();
        }
    }

    ctx.output.emit_json(&report);
    if !ctx.output.json {
        ctx.output.emit_text(serde_json::to_string_pretty(&report).unwrap_or_default());
    }
    Ok(0)
}

fn parse_workflow(value: Option<&str>) -> NodkrayResult<WorkflowKind> {
    match value.unwrap_or("odd").to_ascii_lowercase().as_str() {
        "auto" | "odd" => Ok(WorkflowKind::Odd),
        "st" => Ok(WorkflowKind::St),
        "sdd" => Ok(WorkflowKind::Sdd),
        other => Err(NodkrayError::user_input(
            "INVALID_WORKFLOW",
            format!("unknown workflow '{other}'"),
        )),
    }
}

fn heuristic_effort(description: &str) -> u32 {
    let len = description.len() as u32;
    if len < 40 {
        15
    } else if len < 160 {
        45
    } else {
        75
    }
}
