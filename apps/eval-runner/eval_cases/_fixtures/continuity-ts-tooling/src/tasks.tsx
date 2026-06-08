import { useState } from "react";
import { type Task } from "./storage";

function createTask(title: string, status: Task["status"]): Task {
  return {
    id: Date.now().toString(36),
    title,
    status,
    createdAt: new Date().toISOString(),
  };
}

const statusLabels: Record<Task["status"], string> = {
  todo: "待办",
  doing: "进行中",
  done: "已完成",
};

export default function TaskNotes() {
  const [tasks, setTasks] = useState<Task[]>([]);
  const [newTitle, setNewTitle] = useState("");
  const [newStatus, setNewStatus] = useState<Task["status"]>("todo");

  const addTask = () => {
    const title = newTitle.trim();
    if (!title) return;
    setTasks((items) => [createTask(title, newStatus), ...items]);
    setNewTitle("");
    setNewStatus("todo");
  };

  const markDone = (id: string) => {
    setTasks((items) =>
      items.map((task) => (task.id === id ? { ...task, status: "done" } : task))
    );
  };

  return (
    <section className="card tasks-panel">
      <h2>任务清单</h2>
      <div className="tasks-form">
        <input
          value={newTitle}
          onChange={(event) => setNewTitle(event.target.value)}
          placeholder="输入任务标题"
        />
        <select
          value={newStatus}
          onChange={(event) => setNewStatus(event.target.value as Task["status"])}
        >
          <option value="todo">待办</option>
          <option value="doing">进行中</option>
        </select>
        <button onClick={addTask}>新增</button>
      </div>
      <ul className="tasks-list">
        {tasks.map((task) => (
          <li key={task.id}>
            <span>{task.title}</span>
            <span>{statusLabels[task.status]}</span>
            <span>{task.createdAt}</span>
            {task.status !== "done" && (
              <button onClick={() => markDone(task.id)}>完成</button>
            )}
          </li>
        ))}
      </ul>
    </section>
  );
}
