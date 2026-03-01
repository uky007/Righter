#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CaseOp {
    Lower,
    Upper,
    Toggle,
}

#[derive(Debug, Clone)]
pub enum Motion {
    Line,
    WordForward,
    WordEnd,
    WordBackward,
    LineEnd,
    LineStart,
    FirstNonBlank,
    WORDForward,
    WORDEnd,
    WORDBackward,
    ParagraphForward,
    ParagraphBackward,
    Inner(char),
    Around(char),
    FindForward(char),
    FindBackward(char),
    TillForward(char),
    TillBackward(char),
}

#[derive(Debug, Clone)]
pub enum Command {
    // Movement
    MoveLeft,
    MoveDown,
    MoveUp,
    MoveRight,
    MoveWordForward,
    MoveWordBackward,
    MoveWordEnd,
    MoveLineStart,
    MoveLineEnd,
    MoveFirstNonBlank,
    MoveWORDForward,
    MoveWORDBackward,
    MoveWORDEnd,
    MoveParagraphForward,
    MoveParagraphBackward,

    // Editing
    InsertChar(char),
    DeleteCharForward,
    DeleteCharBackward,
    DeleteLine,
    InsertNewlineBelow,
    InsertNewlineAbove,
    InsertNewline,
    InsertTab,
    IndentLine,
    DedentLine,

    // Operator + motion
    DeleteMotion(Motion),
    ChangeMotion(Motion),
    YankMotion(Motion),

    // Find/till character (standalone motion)
    FindCharForward(char),
    FindCharBackward(char),
    TillCharForward(char),
    TillCharBackward(char),

    // Replace character
    ReplaceChar(char),

    // Join lines
    JoinLines,

    // Undo/Redo
    Undo,
    Redo,

    // Mode changes
    EnterInsertMode,
    EnterInsertModeAfter,
    EnterInsertModeLineEnd,
    EnterInsertModeFirstNonBlank,
    EnterVisualMode,
    EnterVisualLineMode,
    EnterCommandMode,
    ExitToNormalMode,

    // Visual mode operations
    VisualDelete,
    VisualYank,
    VisualChange,
    VisualIndent,
    VisualDedent,
    VisualSwapAnchor,

    // Paste
    PasteAfter,
    PasteBefore,

    // Yank line
    YankLine,

    // Jump list
    JumpBack,
    JumpForward,

    // Completion
    TriggerCompletion,
    AcceptCompletion,
    CancelCompletion,
    CompletionNext,
    CompletionPrev,

    // LSP actions
    GotoDefinition,
    Hover,
    FindReferences,
    DismissPopup,
    ReferenceNext,
    ReferencePrev,
    ReferenceJump,

    // Search
    EnterSearchMode,
    SearchInput(char),
    SearchBackspace,
    SearchConfirm,
    SearchCancel,
    SearchNext,
    SearchPrev,

    // Extended movement
    GotoTop,
    GotoBottom,
    HalfPageDown,
    HalfPageUp,
    FullPageDown,
    FullPageUp,

    // File finder
    OpenFileFinder,
    FileFinderInput(char),
    FileFinderBackspace,
    FileFinderConfirm,
    FileFinderCancel,
    FileFinderNext,
    FileFinderPrev,

    // Phase 9: Repeat
    RepeatLastChange,

    // Phase 9: Search word under cursor
    SearchWordForward,
    SearchWordBackward,

    // Phase 9: Bracket jump
    MatchBracket,

    // Phase 9: Viewport navigation
    ViewportHigh,
    ViewportMiddle,
    ViewportLow,

    // Phase 9: Scroll positioning
    ScrollCenter,
    ScrollTop,
    ScrollBottom,

    // Phase 9: Buffer switching
    NextBuffer,
    PrevBuffer,

    // Phase 10: Case change
    ToggleCaseChar,
    CaseChange(CaseOp, Motion),
    CaseChangeLine(CaseOp),

    // Phase 10: Number increment/decrement
    IncrementNumber,
    DecrementNumber,

    // Phase 10: Named registers
    SelectRegister(char),

    // Phase 10: Macro recording
    StartMacro(char),
    StopMacro,
    PlayMacro(char),
    PlayLastMacro,

    // Phase 10: LSP formatting
    FormatDocument,

    // Phase 11: Diagnostic navigation
    DiagnosticNext,
    DiagnosticPrev,
    DiagnosticList,
    DiagnosticJump,

    // Phase 11: LSP Code Actions
    CodeAction,
    CodeActionNext,
    CodeActionPrev,
    CodeActionAccept,
    CodeActionDismiss,

    // Window split
    SplitHorizontal,
    SplitVertical,
    PaneLeft,
    PaneDown,
    PaneUp,
    PaneRight,
    PaneNext,
    PaneClose,

    // Command mode
    CommandInput(char),
    CommandBackspace,
    CommandExecute,
    CommandHistoryPrev,
    CommandHistoryNext,
}
