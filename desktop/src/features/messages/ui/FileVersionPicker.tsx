import * as React from "react";
import {
  File,
  FileSpreadsheet,
  FileText,
  Folder,
  Link2,
  Presentation,
  Search,
} from "lucide-react";

import {
  type ChannelFileEntry,
  listChannelFiles,
} from "@/shared/api/channelFiles";
import { isGoogleUrl, linkKind } from "@/shared/lib/channelLinkEntries.mjs";
import { Popover, PopoverContent, PopoverTrigger } from "@/shared/ui/popover";

const GENERIC_SURFACE_NAMES = new Set([
  "Doc",
  "Sheet",
  "Slides",
  "Folder",
  "File",
  "Form",
  "Drawing",
  "Meet",
  "Google Doc",
  "Google Sheet",
  "Google Slides",
  "Google Drive folder",
  "Google Drive file",
  "Google Form",
  "Google Drawing",
  "Google Docs file",
  "Google Meet link",
]);

function GoogleGlyph({ className = "h-3.5 w-3.5" }: { className?: string }) {
  return (
    <svg aria-hidden="true" className={className} viewBox="0 0 24 24">
      <path
        d="M23.745 12.27c0-.7-.06-1.4-.19-2.07H12v4.51h6.6c-.29 1.52-1.14 2.82-2.4 3.68v3.05h3.88c2.27-2.09 3.665-5.17 3.665-9.17z"
        fill="#4285F4"
      />
      <path
        d="M12 24c3.24 0 5.95-1.08 7.93-2.91l-3.88-3.05c-1.08.72-2.45 1.16-4.05 1.16-3.12 0-5.77-2.1-6.72-4.93H1.25v3.15C3.26 21.36 7.33 24 12 24z"
        fill="#34A853"
      />
      <path
        d="M5.28 14.27c-.25-.72-.38-1.49-.38-2.27s.13-1.55.38-2.27V6.58H1.25C.45 8.18 0 10.03 0 12s.45 3.82 1.25 5.42l4.03-3.15z"
        fill="#FBBC05"
      />
      <path
        d="M12 4.75c1.77 0 3.35.61 4.6 1.8l3.42-3.42C17.95 1.19 15.24 0 12 0 7.33 0 3.26 2.64 1.25 6.58l4.03 3.15c.95-2.83 3.6-4.98 6.72-4.98z"
        fill="#EA4335"
      />
    </svg>
  );
}

function renderTypeIcon(file: ChannelFileEntry) {
  if (file.kind === "file") {
    const name = (file.filename ?? "").toLowerCase();
    if (/\.(pptx?|key)$/.test(name)) {
      return <Presentation className="h-3.5 w-3.5 shrink-0 text-muted-foreground" />;
    }
    if (/\.(xlsx?|csv|tsv)$/.test(name)) {
      return <FileSpreadsheet className="h-3.5 w-3.5 shrink-0 text-muted-foreground" />;
    }
    if (/\.(docx?|pdf|txt|md)$/.test(name)) {
      return <FileText className="h-3.5 w-3.5 shrink-0 text-muted-foreground" />;
    }
    return <File className="h-3.5 w-3.5 shrink-0 text-muted-foreground" />;
  }
  const kind = linkKind(file.url);
  if (kind === "folder") {
    return <Folder className="h-3.5 w-3.5 shrink-0 text-muted-foreground" />;
  }
  if (kind === "slides") {
    return <Presentation className="h-3.5 w-3.5 shrink-0 text-muted-foreground" />;
  }
  if (kind === "sheet") {
    return <FileSpreadsheet className="h-3.5 w-3.5 shrink-0 text-muted-foreground" />;
  }
  if (kind === "doc") {
    return <FileText className="h-3.5 w-3.5 shrink-0 text-muted-foreground" />;
  }
  return <Link2 className="h-3.5 w-3.5 shrink-0 text-muted-foreground" />;
}

/** When the file was shared, `dd-mm-yy hh:mm` (24-hour), so two same-named
 * entries (e.g. two "Google Drive folder" links) can be told apart.
 * `uploadedAt` is Unix seconds. */
function formatUploadedAt(seconds: number): string {
  if (!Number.isFinite(seconds) || seconds <= 0) return "";
  const d = new Date(seconds * 1000);
  const p = (n: number) => String(n).padStart(2, "0");
  return `${p(d.getDate())}-${p(d.getMonth() + 1)}-${String(d.getFullYear()).slice(-2)} ${p(d.getHours())}:${p(d.getMinutes())}`;
}

export type FileVersionPickerExclusion = {
  /** Exclude the candidate sharing this sha256 — a file can't supersede a
   * byte-identical copy of itself. */
  sha256?: string | null;
  /** Exclude the candidate with this event id — a file can't supersede
   * itself. */
  eventId?: string | null;
};

type FileVersionPickerProps = {
  channelId: string;
  exclude?: FileVersionPickerExclusion;
  onOpenChange: (open: boolean) => void;
  onSelect: (file: ChannelFileEntry) => void;
  open: boolean;
  trigger: React.ReactNode;
};

/**
 * Small searchable popover listing a channel's current (non-superseded)
 * files, for manually linking something as a "new version of" one of them.
 * Shared by two entry points:
 *
 *   - the composer's per-attachment "Link to a different file…" affordance
 *     (`ComposerAttachments.tsx`), picking an earlier upload for an
 *     attachment that hasn't been sent yet;
 *   - the Files tab's per-row retroactive-link action (`FilesPanel.tsx`),
 *     picking an earlier upload for a file that was already sent.
 *
 * Reuses the same `Popover` primitive as `ComposerEmojiPicker` and the
 * mention/channel autocompletes rather than inventing new popover machinery.
 * Fetches `listChannelFiles` lazily on open (not on mount), since the full
 * page-through-history list can be sizeable and most attachments never open
 * the picker.
 */
export function FileVersionPicker({
  channelId,
  exclude,
  onOpenChange,
  onSelect,
  open,
  trigger,
}: FileVersionPickerProps) {
  const [query, setQuery] = React.useState("");
  const [files, setFiles] = React.useState<ChannelFileEntry[] | null>(null);
  const [loadError, setLoadError] = React.useState(false);

  React.useEffect(() => {
    if (!open) return;
    setQuery("");
    setFiles(null);
    setLoadError(false);
    let cancelled = false;
    void (async () => {
      try {
        const result = await listChannelFiles(channelId);
        if (!cancelled) setFiles(result);
      } catch {
        if (!cancelled) setLoadError(true);
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [open, channelId]);

  const candidates = React.useMemo(() => {
    if (!files) return [];
    const normalizedQuery = query.trim().toLowerCase();
    const filtered = files.filter((file) => {
      if (file.filename == null && file.note == null) return false;
      if (file.supersededBy != null) return false; // already outdated
      if (exclude?.eventId && file.eventId === exclude.eventId) return false;
      if (exclude?.sha256 && file.sha256 && file.sha256 === exclude.sha256) {
        return false;
      }
      if (!normalizedQuery) return true;
      return (
        (file.filename?.toLowerCase().includes(normalizedQuery) ?? false) ||
        (file.note?.toLowerCase().includes(normalizedQuery) ?? false)
      );
    });
    return filtered.sort((a, b) => (b.uploadedAt ?? 0) - (a.uploadedAt ?? 0));
  }, [files, query, exclude?.eventId, exclude?.sha256]);

  return (
    <Popover onOpenChange={onOpenChange} open={open}>
      <PopoverTrigger asChild>{trigger}</PopoverTrigger>
      <PopoverContent
        align="start"
        className="w-80 p-2"
        data-testid="file-version-picker"
        side="top"
        sideOffset={8}
      >
        <div className="flex items-center gap-1.5 rounded-lg border border-border/60 bg-background px-2 py-1">
          <Search className="h-3.5 w-3.5 shrink-0 text-muted-foreground" />
          <input
            // biome-ignore lint/a11y/noAutofocus: popover search
            autoFocus
            className="w-full bg-transparent text-xs outline-hidden placeholder:text-muted-foreground"
            data-testid="file-version-picker-search"
            onChange={(event) => setQuery(event.target.value)}
            placeholder="Search files…"
            value={query}
          />
        </div>
        <div className="mt-1.5 max-h-48 overflow-y-auto">
          {files === null && !loadError ? (
            <div className="px-2 py-1.5 text-2xs text-muted-foreground">
              Loading…
            </div>
          ) : loadError ? (
            <div className="px-2 py-1.5 text-2xs text-muted-foreground">
              Couldn't load this channel's files.
            </div>
          ) : candidates.length === 0 ? (
            <div className="px-2 py-1.5 text-2xs text-muted-foreground">
              No matching files.
            </div>
          ) : (
            candidates.map((file) => {
              const isGoogle = file.url ? isGoogleUrl(file.url) : false;
              const hasGenericName =
                file.kind === "link" &&
                (!file.filename || GENERIC_SURFACE_NAMES.has(file.filename));
              // Option C: If a note exists, show only the note (no generic label);
              // if no note exists, show Folder, Slides, Sheet, Doc, etc.
              const primaryText =
                hasGenericName && file.note
                  ? file.note
                  : (file.filename ?? file.note ?? "");
              const secondaryNote =
                hasGenericName || !file.note ? null : file.note;

              return (
                <button
                  className="flex w-full items-start gap-2 rounded-lg px-2 py-1.5 text-left text-xs text-popover-foreground hover:bg-accent/50"
                  data-testid="file-version-picker-option"
                  key={file.eventId}
                  onMouseDown={(event) => {
                    event.preventDefault();
                    onSelect(file);
                    onOpenChange(false);
                  }}
                  title={primaryText || undefined}
                  type="button"
                >
                  <div className="mt-0.5 flex shrink-0 items-center gap-1">
                    {isGoogle && <GoogleGlyph className="h-3.5 w-3.5 shrink-0" />}
                    {renderTypeIcon(file)}
                  </div>
                  <span className="flex min-w-0 flex-1 flex-col">
                    <span className="break-words leading-tight">{primaryText}</span>
                    {secondaryNote && (
                      <span
                        className="break-words text-2xs text-muted-foreground"
                        title={secondaryNote}
                      >
                        {secondaryNote}
                      </span>
                    )}
                    {file.uploadedAt > 0 && (
                      <span className="text-2xs text-muted-foreground/80">
                        {formatUploadedAt(file.uploadedAt)}
                      </span>
                    )}
                  </span>
                </button>
              );
            })
          )}
        </div>
      </PopoverContent>
    </Popover>
  );
}
