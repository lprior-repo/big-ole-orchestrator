import fs from "node:fs";

const file = ".decomposition/session_1775156208/draft-tasks.json";
const defectsFile = ".decomposition/session_1775156208/defects.md";
const data = JSON.parse(fs.readFileSync(file, "utf8"));
const allowedEffort = new Set(["15min", "30min", "1hr", "2hr"]);
const requiredTop = [
  "id",
  "title",
  "type",
  "priority",
  "effort",
  "description",
  "clarifications",
  "ears",
  "contracts",
  "tests",
  "research",
  "implementation",
  "context"
];

const defects = [];

for (const task of data) {
  for (const key of requiredTop) {
    if (!(key in task)) {
      defects.push(`${task.id}: missing required field ${key}.`);
    }
  }
  if (!allowedEffort.has(task.effort)) {
    defects.push(`${task.id}: invalid effort ${task.effort}.`);
  }
  if (/\band\b/i.test(task.title)) {
    defects.push(`${task.id}: title contains 'and', which smells like bundled work.`);
  }
  if (task.research?.files?.length > 3) {
    defects.push(`${task.id}: research.files exceeds 3 entries, violating blast radius discipline.`);
  }
  for (const section of ["resolved", "open", "assumptions"]) {
    if (!Array.isArray(task.clarifications?.[section])) {
      defects.push(`${task.id}: clarifications.${section} must be an array.`);
    }
  }
  if (!Array.isArray(task.ears?.ubiquitous) || !Array.isArray(task.ears?.event_driven) || !Array.isArray(task.ears?.unwanted)) {
    defects.push(`${task.id}: EARS arrays are incomplete.`);
  }
  for (const section of ["preconditions", "postconditions", "invariants"]) {
    if (!Array.isArray(task.contracts?.[section])) {
      defects.push(`${task.id}: contracts.${section} must be an array.`);
    }
  }
  for (const section of ["happy", "error", "edge"]) {
    if (!Array.isArray(task.tests?.[section])) {
      defects.push(`${task.id}: tests.${section} must be an array.`);
    }
  }
  for (const section of ["phase_0", "phase_1", "phase_2"]) {
    if (!Array.isArray(task.implementation?.[section])) {
      defects.push(`${task.id}: implementation.${section} must be an array.`);
    }
  }
  for (const section of ["related_files", "similar"]) {
    if (!Array.isArray(task.context?.[section])) {
      defects.push(`${task.id}: context.${section} must be an array.`);
    }
  }
}

if (defects.length > 0) {
  fs.writeFileSync(defectsFile, defects.map((line) => `- ${line}`).join("\n") + "\n");
  process.stdout.write("STATUS: REJECTED\n");
  process.stdout.write(defects.join("\n") + "\n");
} else {
  process.stdout.write("STATUS: APPROVED\n");
}
