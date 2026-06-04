import { useState } from "react";
import TaskNotes from "./tasks";

export default function App() {
  const [count, setCount] = useState(0);

  return (
    <div className="app">
      <h1>Continuity Eval Fixture</h1>
      <p>这个小项目用于把真实 Forge session 转成可重复回测。</p>

      <div className="card">
        <button onClick={() => setCount((value) => value + 1)}>
          Clicked {count} time{count !== 1 ? "s" : ""}
        </button>
      </div>

      <TaskNotes />
    </div>
  );
}
