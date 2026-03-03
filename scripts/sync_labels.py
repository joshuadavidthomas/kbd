#!/usr/bin/env -S uv run
# /// script
# requires-python = ">=3.12"
# dependencies = [
#   "pyyaml"
# ]
# ///
"""Sync GitHub repo labels with .github/labels.yml."""

from __future__ import annotations

import json
import subprocess
from dataclasses import dataclass
from enum import Enum
from enum import auto
from pathlib import Path

import yaml


@dataclass(frozen=True)
class Label:
    name: str
    color: str
    description: str

    @classmethod
    def from_yaml(cls, data: dict[str, str]) -> Label:
        return cls(
            name=data["name"],
            color=data["color"].lstrip("#").lower(),
            description=data["description"],
        )

    @classmethod
    def from_gh(cls, data: dict[str, str]) -> Label:
        return cls(
            name=data["name"],
            color=data["color"].lower(),
            description=data["description"],
        )

    def matches_content(self, other: Label) -> bool:
        return self.color == other.color and self.description == other.description

    def base_names(self) -> set[str]:
        """Possible base names with emoji stripped (prefix or suffix)."""
        names = {self.name}
        parts = self.name.rsplit(" ", 1)
        if len(parts) == 2:
            names.add(parts[0])
        parts = self.name.split(" ", 1)
        if len(parts) == 2:
            names.add(parts[1])
        return names


class ActionKind(Enum):
    CREATE = auto()
    UPDATE = auto()
    DELETE = auto()


@dataclass(frozen=True)
class Action:
    kind: ActionKind
    label: Label
    old_name: str | None = None

    def execute(self) -> None:
        match self.kind:
            case ActionKind.CREATE:
                print(f"  create: '{self.label.name}'")
                gh(
                    "label",
                    "create",
                    self.label.name,
                    "--color",
                    self.label.color,
                    "--description",
                    self.label.description,
                )
            case ActionKind.UPDATE:
                old = self.old_name or self.label.name
                print(f"  update: '{old}' -> '{self.label.name}'")
                gh(
                    "label",
                    "edit",
                    old,
                    "--name",
                    self.label.name,
                    "--color",
                    self.label.color,
                    "--description",
                    self.label.description,
                )
            case ActionKind.DELETE:
                print(f"  delete: '{self.label.name}'")
                gh("label", "delete", self.label.name, "--yes")


def gh(*args: str) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        ["gh", *args],
        capture_output=True,
        text=True,
        check=True,
    )


def diff(desired: list[Label], current: list[Label]) -> list[Action]:
    current_by_name: dict[str, Label] = {l.name: l for l in current}
    matched: set[str] = set()
    actions: list[Action] = []

    for d in desired:
        if d.name in current_by_name:
            matched.add(d.name)
            if not d.matches_content(current_by_name[d.name]):
                actions.append(Action(ActionKind.UPDATE, d))
            else:
                print(f"  ok: {d.name}")
            continue

        rename_from = find_rename_candidate(d, current, matched)
        if rename_from is not None:
            matched.add(rename_from)
            actions.append(Action(ActionKind.UPDATE, d, old_name=rename_from))
        else:
            actions.append(Action(ActionKind.CREATE, d))

    for c in current:
        if c.name not in matched:
            actions.append(Action(ActionKind.DELETE, c))

    return actions


def find_rename_candidate(
    desired: Label, current: list[Label], matched: set[str]
) -> str | None:
    bases = desired.base_names()
    for c in current:
        if c.name not in matched and c.name in bases:
            return c.name
    return None


def main() -> None:
    repo_root = Path(__file__).resolve().parent.parent
    labels_file = repo_root / ".github" / "labels.yml"

    with open(labels_file) as f:
        desired = [Label.from_yaml(entry) for entry in yaml.safe_load(f)]

    result = gh("label", "list", "--json", "name,color,description", "--limit", "100")
    current = [Label.from_gh(entry) for entry in json.loads(result.stdout)]

    actions = diff(desired, current)
    for action in actions:
        action.execute()

    if not actions:
        print("\nAll labels in sync.")
    else:
        print(f"\n{len(actions)} label(s) synced.")


if __name__ == "__main__":
    main()
