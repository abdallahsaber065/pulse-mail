export interface SyntaxDoc {
  operator: string;
  syntax: string;
  description: string;
  example: string;
  category: "Filters" | "People & Subject" | "Location" | "Attachments" | "Date & Size";
}

export const SYNTAX_GUIDE: SyntaxDoc[] = [
  // Filters
  { operator: "is:unread", syntax: "is:unread", description: "Unread emails", example: "is:unread", category: "Filters" },
  { operator: "is:read", syntax: "is:read", description: "Read emails", example: "is:read", category: "Filters" },
  { operator: "is:starred", syntax: "is:starred", description: "Starred messages", example: "is:starred", category: "Filters" },
  { operator: "is:important", syntax: "is:important", description: "Messages marked important", example: "is:important", category: "Filters" },

  // People & Subject
  { operator: "from:", syntax: "from:<name or email>", description: "Messages sent by a person", example: "from:google", category: "People & Subject" },
  { operator: "to:", syntax: "to:<name or email>", description: "Messages sent to someone", example: "to:me", category: "People & Subject" },
  { operator: "subject:", syntax: "subject:<word>", description: "Words in the subject line", example: "subject:invoice", category: "People & Subject" },

  // Location & Categories
  { operator: "in:inbox", syntax: "in:inbox", description: "Search in Inbox", example: "in:inbox", category: "Location" },
  { operator: "in:sent", syntax: "in:sent", description: "Search sent mail", example: "in:sent", category: "Location" },
  { operator: "in:draft", syntax: "in:draft", description: "Search drafts", example: "in:draft", category: "Location" },
  { operator: "in:trash", syntax: "in:trash", description: "Search deleted mail", example: "in:trash", category: "Location" },
  { operator: "in:spam", syntax: "in:spam", description: "Search spam folder", example: "in:spam", category: "Location" },
  { operator: "in:anywhere", syntax: "in:anywhere", description: "Search across all folders", example: "in:anywhere", category: "Location" },
  { operator: "label:", syntax: "label:<name>", description: "Search by user or system label", example: "label:Work", category: "Location" },
  { operator: "category:primary", syntax: "category:primary", description: "Primary inbox tab", example: "category:primary", category: "Location" },
  { operator: "category:promotions", syntax: "category:promotions", description: "Promotions tab", example: "category:promotions", category: "Location" },
  { operator: "category:social", syntax: "category:social", description: "Social tab", example: "category:social", category: "Location" },
  { operator: "category:updates", syntax: "category:updates", description: "Updates tab", example: "category:updates", category: "Location" },
  { operator: "category:forums", syntax: "category:forums", description: "Forums tab", example: "category:forums", category: "Location" },

  // Attachments
  { operator: "has:attachment", syntax: "has:attachment", description: "Emails with files attached", example: "has:attachment", category: "Attachments" },
  { operator: "has:drive", syntax: "has:drive", description: "Emails with Google Drive links", example: "has:drive", category: "Attachments" },
  { operator: "has:document", syntax: "has:document", description: "Emails with docs attached", example: "has:document", category: "Attachments" },
  { operator: "filename:", syntax: "filename:<pdf|zip|name>", description: "Attachment name or extension", example: "filename:pdf", category: "Attachments" },

  // Date & Size
  { operator: "newer_than:", syntax: "newer_than:<7d|1m|1y>", description: "Messages newer than days/months/years", example: "newer_than:7d", category: "Date & Size" },
  { operator: "older_than:", syntax: "older_than:<7d|1m|1y>", description: "Messages older than days/months/years", example: "older_than:1y", category: "Date & Size" },
  { operator: "after:", syntax: "after:YYYY/MM/DD", description: "Messages received after a date", example: "after:2026/01/01", category: "Date & Size" },
  { operator: "before:", syntax: "before:YYYY/MM/DD", description: "Messages received before a date", example: "before:2026/01/01", category: "Date & Size" },
  { operator: "larger:", syntax: "larger:<5M|500K>", description: "Messages larger than size", example: "larger:5M", category: "Date & Size" },
  { operator: "smaller:", syntax: "smaller:<5M|500K>", description: "Messages smaller than size", example: "smaller:1M", category: "Date & Size" },
];

export interface SearchSuggestion {
  token: string;
  syntax: string;
  description: string;
  example: string;
}

/**
 * Extracts the token being actively typed under cursor, or at the end of the query.
 */
export function getActiveToken(query: string, cursorIndex?: number): { token: string; start: number; end: number } {
  const pos = cursorIndex != null ? Math.min(Math.max(cursorIndex, 0), query.length) : query.length;
  const before = query.slice(0, pos);
  
  // Find start of current word
  const match = /\S+$/u.exec(before);
  if (!match) {
    return { token: "", start: pos, end: pos };
  }
  const start = match.index;
  // Find end of current word
  const after = query.slice(pos);
  const afterMatch = /^\S+/u.exec(after);
  const end = pos + (afterMatch ? afterMatch[0].length : 0);
  
  return {
    token: query.slice(start, end),
    start,
    end,
  };
}

/**
 * Returns autocomplete syntax suggestions based on what the user is typing.
 */
export function getSearchSuggestions(query: string, cursorIndex?: number): SearchSuggestion[] {
  const { token } = getActiveToken(query, cursorIndex);
  const trimmed = token.trim().toLowerCase();

  // If empty or whitespace, return popular quick filters
  if (!trimmed) {
    return [
      { token: "is:unread", syntax: "is:unread", description: "Unread emails", example: "is:unread" },
      { token: "is:starred", syntax: "is:starred", description: "Starred emails", example: "is:starred" },
      { token: "has:attachment", syntax: "has:attachment", description: "Has attached files", example: "has:attachment" },
      { token: "from:", syntax: "from:<sender>", description: "Search by sender", example: "from:google" },
      { token: "in:inbox", syntax: "in:inbox", description: "In Inbox", example: "in:inbox" },
      { token: "category:primary", syntax: "category:primary", description: "Primary tab", example: "category:primary" },
    ];
  }

  // Exact matching or prefix matching
  const matches = SYNTAX_GUIDE.filter((item) => {
    const op = item.operator.toLowerCase();
    const syn = item.syntax.toLowerCase();
    return op.startsWith(trimmed) || syn.startsWith(trimmed) || (trimmed.length >= 2 && item.description.toLowerCase().includes(trimmed));
  });

  return matches.map((m) => ({
    token: m.operator,
    syntax: m.syntax,
    description: m.description,
    example: m.example,
  }));
}

/**
 * Replaces the active token in the query with the selected suggestion.
 */
export function applySuggestion(query: string, suggestion: string, cursorIndex?: number): { newQuery: string; newCursor: number } {
  const { start, end } = getActiveToken(query, cursorIndex);
  const before = query.slice(0, start);
  const after = query.slice(end);

  const insert = suggestion.endsWith(":") ? suggestion : `${suggestion} `;
  const newQuery = `${before}${insert}${after}`.replace(/\s{2,}/g, " ").trimStart();
  const newCursor = before.length + insert.length;

  return { newQuery, newCursor };
}

