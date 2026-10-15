import { fireEvent, screen } from "@testing-library/react";

/** The composer prompt is a contentEditable textbox; Image N chips serialize back to plain "Image N" text. */
export function promptEditor(name = "提示词") { return screen.getByRole("textbox", { name }); }
export function setPrompt(text: string, name = "提示词") { const el = promptEditor(name); el.textContent = text; fireEvent.input(el); }
export function promptText(name = "提示词") { return promptEditor(name).textContent ?? ""; }
