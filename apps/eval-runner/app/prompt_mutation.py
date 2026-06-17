from app.models import EvaluationTask


def mutate_prompt(task: EvaluationTask, *, style: str) -> EvaluationTask:
    if style == "terse-bug-report":
        prompt = f"This is broken. Please fix {task.prompt}"
    elif style == "chatty-ambiguous":
        prompt = (
            "I was trying this workflow and it feels off. Can you take a look "
            f"and make it work? Details: {task.prompt}"
        )
    elif style == "constraint-heavy":
        prompt = (
            f"{task.prompt}\n"
            "Keep changes minimal, run validation, and do not touch forbidden files."
        )
    else:
        raise ValueError(f"Unknown prompt mutation style: {style}")

    metadata = {
        **task.metadata,
        "base_task_id": task.id,
        "mutation_style": style,
    }
    return task.model_copy(
        update={
            "id": f"{task.id}__{style}",
            "prompt": prompt,
            "metadata": metadata,
        }
    )
