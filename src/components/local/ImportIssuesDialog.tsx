import { Button, Dialog, DialogClose, DialogContent } from "@/components/ui";
import { strings } from "@/i18n";
import { reasonLabel } from "@/lib/local";
import { useLocalImportStore } from "@/stores/localImport";

const fileName = (path: string) => path.split(/[\\/]/).pop() ?? path;

/** "Files not added": each file with its reason (opened from the import toast). */
export function ImportIssuesDialog() {
  const open = useLocalImportStore((s) => s.issuesOpen);
  const issues = useLocalImportStore((s) => s.issues);
  const close = useLocalImportStore((s) => s.closeIssues);
  return (
    <Dialog open={open} onOpenChange={(next) => (next ? undefined : close())}>
      <DialogContent
        title={strings.localImport.detailsTitle}
        footer={
          <DialogClose asChild>
            <Button variant="secondary">{strings.common.ok}</Button>
          </DialogClose>
        }
      >
        <ul className="-mx-1 max-h-72 overflow-y-auto">
          {issues.map((issue) => (
            <li key={issue.path} className="flex flex-col gap-0.5 rounded-[8px] px-1 py-1.5" title={issue.path}>
              <span className="truncate text-body text-text">{fileName(issue.path)}</span>
              <span className="text-footnote text-text-muted">{reasonLabel(issue.reason)}</span>
            </li>
          ))}
        </ul>
      </DialogContent>
    </Dialog>
  );
}
