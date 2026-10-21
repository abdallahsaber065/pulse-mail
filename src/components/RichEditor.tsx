// Compose body editor: TipTap (ProseMirror) with a small formatting set
// (ADR 0020). Paste from Word/web keeps only what the schema allows
// (paragraphs, lists, quotes, links, bold/italic/underline/strike, code);
// Rust sanitizes the HTML again before sending.
import { Editor } from "@tiptap/core";
import StarterKit from "@tiptap/starter-kit";
import { createSignal, For, onCleanup, onMount, Show } from "solid-js";
import Icon, { type IconName } from "./Icon";

export interface EditorApi {
  focus(atStart?: boolean): void;
}

interface Props {
  html: string;
  onChange(html: string, text: string): void;
  ref?(api: EditorApi): void;
}

type Mark = "bold" | "italic" | "underline" | "strike" | "bulletList" | "orderedList" | "blockquote" | "link";

export default function RichEditor(p: Props) {
  let host!: HTMLDivElement;
  let editor: Editor | undefined;
  const [active, setActive] = createSignal<Set<Mark>>(new Set());
  const [linking, setLinking] = createSignal<string | null>(null);

  const refresh = () => {
    if (!editor) return;
    const marks: Mark[] = ["bold", "italic", "underline", "strike", "bulletList", "orderedList", "blockquote", "link"];
    setActive(new Set(marks.filter((m) => editor!.isActive(m))));
  };

  const openLink = () => {
    if (!editor) return;
    setLinking((editor.getAttributes("link").href as string | undefined) ?? "https://");
  };

  const applyLink = (href: string) => {
    if (!editor) return;
    const url = href.trim();
    const chain = editor.chain().focus().extendMarkRange("link");
    if (!url || url === "https://") chain.unsetLink().run();
    else if (/^(https?:|mailto:)/i.test(url)) chain.setLink({ href: url }).run();
    else chain.setLink({ href: `https://${url}` }).run();
    setLinking(null);
  };

  onMount(() => {
    editor = new Editor({
      element: host,
      textDirection: "auto",
      extensions: [
        StarterKit.configure({
          heading: { levels: [2, 3] },
          link: { openOnClick: false, autolink: true, protocols: ["http", "https", "mailto"], defaultProtocol: "https" },
        }),
      ],
      content: p.html,
      editorProps: {
        attributes: { class: "no-ring pm-editor", "aria-label": "Message", role: "textbox", "aria-multiline": "true", spellcheck: "true" },
        handleKeyDown: (_view, e) => {
          if ((e.ctrlKey || e.metaKey) && !e.shiftKey && e.key.toLowerCase() === "k") {
            e.preventDefault();
            openLink();
            return true;
          }
          return false;
        },
      },
      onUpdate: ({ editor: ed }) => {
        p.onChange(ed.isEmpty ? "" : ed.getHTML(), ed.getText({ blockSeparator: "\n\n" }));
      },
      onTransaction: refresh,
    });
    p.ref?.({ focus: (atStart) => editor?.commands.focus(atStart ? "start" : "end") });
  });
  onCleanup(() => editor?.destroy());

  const btn = (mark: Mark, icon: IconName, title: string, run: () => void) => ({ mark, icon, title, run });
  const tools = () => [
    btn("bold", "bold", "Bold (Ctrl+B)", () => editor?.chain().focus().toggleBold().run()),
    btn("italic", "italic", "Italic (Ctrl+I)", () => editor?.chain().focus().toggleItalic().run()),
    btn("underline", "underline", "Underline (Ctrl+U)", () => editor?.chain().focus().toggleUnderline().run()),
    btn("strike", "strike", "Strikethrough (Ctrl+Shift+S)", () => editor?.chain().focus().toggleStrike().run()),
    btn("bulletList", "bulletList", "Bulleted list (Ctrl+Shift+8)", () => editor?.chain().focus().toggleBulletList().run()),
    btn("orderedList", "numberList", "Numbered list (Ctrl+Shift+7)", () => editor?.chain().focus().toggleOrderedList().run()),
    btn("blockquote", "quote", "Quote (Ctrl+Shift+B)", () => editor?.chain().focus().toggleBlockquote().run()),
    btn("link", "link", "Link (Ctrl+K)", openLink),
  ];

  return (
    <div class="flex min-h-0 flex-1 flex-col">
      <div ref={host} class="min-h-[140px] flex-1 text-sm leading-6 text-text" />
      <Show when={linking() != null}>
        <form
          class="mt-2 flex items-center gap-2 rounded-lg bg-surface-container px-2 py-1"
          onSubmit={(e) => {
            e.preventDefault();
            applyLink(linking() ?? "");
          }}
        >
          <Icon name="link" size={16} class="text-muted" />
          <input
            class="no-ring h-8 min-w-0 flex-1 bg-transparent text-sm text-text outline-none"
            value={linking() ?? ""}
            onInput={(e) => setLinking(e.currentTarget.value)}
            onKeyDown={(e) => {
              if (e.key === "Escape") {
                e.preventDefault();
                e.stopPropagation();
                setLinking(null);
                editor?.commands.focus();
              }
            }}
            aria-label="Link address"
            ref={(el) => queueMicrotask(() => el.select())}
          />
          <button type="submit" class="h-7 rounded-full px-3 text-xs font-medium text-primary hover:bg-primary/10">
            Apply
          </button>
          <button type="button" class="h-7 rounded-full px-2 text-xs text-muted hover:bg-hover" onClick={() => applyLink("")}>
            Remove
          </button>
        </form>
      </Show>
      <div class="mt-2 flex shrink-0 items-center gap-0.5" role="toolbar" aria-label="Formatting">
        <For each={tools()}>
          {(t) => (
            <button
              type="button"
              class={`grid size-8 place-items-center rounded-md ${active().has(t.mark) ? "bg-secondary-container text-on-secondary-container" : "text-muted hover:bg-hover"}`}
              title={t.title}
              aria-label={t.title}
              aria-pressed={active().has(t.mark)}
              onMouseDown={(e) => e.preventDefault()}
              onClick={t.run}
            >
              <Icon name={t.icon} size={18} />
            </button>
          )}
        </For>
        <button
          type="button"
          class="grid size-8 place-items-center rounded-md text-muted hover:bg-hover"
          title="Clear formatting"
          aria-label="Clear formatting"
          onMouseDown={(e) => e.preventDefault()}
          onClick={() => editor?.chain().focus().unsetAllMarks().clearNodes().run()}
        >
          <Icon name="format_clear" size={18} />
        </button>
      </div>
    </div>
  );
}
