import en from "./en.json";

export const strings = en;

/** Replaces `{name}` placeholders: format("Added to {name}", { name: "Mix" }). */
export function format(template: string, vars: Record<string, string | number>): string {
  return template.replace(/\{(\w+)\}/g, (match, key: string) =>
    key in vars ? String(vars[key]) : match,
  );
}
