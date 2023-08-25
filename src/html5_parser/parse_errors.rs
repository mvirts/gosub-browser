pub enum ParserError {
    AbruptDoctypePublicIdentifier,
    AbruptDoctypeSystemIdentifier,
    AbruptClosingOfEmptyComment,
    AbsenceOfDigitsInNumericCharacterReference,
    CdataInHtmlContent,
    CharacterReferenceOutsideUnicodeRange,
    ControlCharacterInInputStream,
    ControlCharacterReference,
    EndTagWithAttributes,
    DuplicateAttribute,
    EndTagWithTrailingSolidus,
    EofBeforeTagName,
    EofInCdata,
    EofInComment,
    EofInDoctype,
    EofInScriptHtmlCommentLikeText,
    EofInTag,
    IncorrectlyClosedComment,
    IncorrectlyOpenedComment,
    InvalidCharacterSequenceAfterDoctypeName,
    InvalidFirstCharacterOfTagName,
    MissingAttributeValue,
    MissingDoctypeName,
    MissingDoctypePublicIdentifier,
    MissingDoctypeSystemIdentifier,
    MissingEndTagName,
    MissingQuoteBeforeDoctypePublicIdentifier,
    MissingQuoteBeforeDoctypeSystemIdentifier,
    MissingSemicolonAfterCharacterReference,
    MissingWhitespaceAfterDoctypePublicKeyword,
    MissingWhitespaceAfterDoctypeSystemKeyword,
    MissingWhitespaceBeforeDoctypeName,
    MissingWhitespaceBetweenAttributes,
    MissingWhitespaceBetweenDoctypePublicAndSystemIdentifiers,
    NestedComment,
    NoncharacterCharacterReference,
    NoncharacterInInputStream,
    NonVoidHtmlElementStartTagWithTrailingSolidus,
    NullCharacterReference,
    SurrogateCharacterReference,
    SurrogateInInputStream,
    UnexpectedCharacterAfterDoctypeSystemIdentifier,
    UnexpectedCharacterInAttributeName,
    UnexpectedCharacterInUnquotedAttributeValue,
    UnexpectedEqualsSignBeforeAttributeName,
    UnexpectedNullCharacter,
    UnexpectedQuestionMarkInsteadOfTagName,
    UnexpectedSolidusInTag,
    UnknownNamedCharacterReference,
}

impl ParserError {
    pub fn as_str(&self) -> &'static str {
        match self {
            ParserError::AbruptDoctypePublicIdentifier => "abrupt-doctype-public-identifier",
            ParserError::AbruptDoctypeSystemIdentifier => "abrupt-doctype-system-identifier",
            ParserError::AbsenceOfDigitsInNumericCharacterReference => "absence-of-digits-in-numeric-character-reference",
            ParserError::CdataInHtmlContent => "cdata-in-html-content",
            ParserError::CharacterReferenceOutsideUnicodeRange => "character-reference-outside-unicode-range",
            ParserError::ControlCharacterInInputStream => "control-character-in-input-stream",
            ParserError::ControlCharacterReference => "control-character-reference",
            ParserError::EndTagWithAttributes => "end-tag-with-attributes",
            ParserError::DuplicateAttribute => "duplicate-attribute",
            ParserError::EndTagWithTrailingSolidus => "end-tag-with-trailing-solidus",
            ParserError::EofBeforeTagName => "eof-before-tag-name",
            ParserError::EofInCdata => "eof-in-cdata",
            ParserError::EofInComment => "eof-in-comment",
            ParserError::EofInDoctype => "eof-in-doctype",
            ParserError::EofInScriptHtmlCommentLikeText => "eof-in-script-html-comment-like-text",
            ParserError::EofInTag => "eof-in-tag",
            ParserError::IncorrectlyClosedComment => "incorrectly-closed-comment",
            ParserError::IncorrectlyOpenedComment => "incorrectly-opened-comment",
            ParserError::InvalidCharacterSequenceAfterDoctypeName => "invalid-character-sequence-after-doctype-name",
            ParserError::InvalidFirstCharacterOfTagName => "invalid-first-character-of-tag-name",
            ParserError::MissingAttributeValue => "missing-attribute-value",
            ParserError::MissingDoctypeName => "missing-doctype-name",
            ParserError::MissingDoctypePublicIdentifier => "missing-doctype-public-identifier",
            ParserError::MissingDoctypeSystemIdentifier => "missing-doctype-system-identifier",
            ParserError::MissingEndTagName => "missing-end-tag-name",
            ParserError::MissingQuoteBeforeDoctypePublicIdentifier => "missing-quote-before-doctype-public-identifier",
            ParserError::MissingQuoteBeforeDoctypeSystemIdentifier => "missing-quote-before-doctype-system-identifier",
            ParserError::MissingSemicolonAfterCharacterReference => "missing-semicolon-after-character-reference",
            ParserError::MissingWhitespaceAfterDoctypePublicKeyword => "missing-whitespace-after-doctype-public-keyword",
            ParserError::MissingWhitespaceAfterDoctypeSystemKeyword => "missing-whitespace-after-doctype-system-keyword",
            ParserError::MissingWhitespaceBeforeDoctypeName => "missing-whitespace-before-doctype-name",
            ParserError::MissingWhitespaceBetweenAttributes => "missing-whitespace-between-attributes",
            ParserError::MissingWhitespaceBetweenDoctypePublicAndSystemIdentifiers => "missing-whitespace-between-doctype-public-and-system-identifiers",
            ParserError::NestedComment => "nested-comment",
            ParserError::NoncharacterCharacterReference => "noncharacter-character-reference",
            ParserError::NoncharacterInInputStream => "noncharacter-in-input-stream",
            ParserError::NonVoidHtmlElementStartTagWithTrailingSolidus => "non-void-html-element-start-tag-with-trailing-solidus",
            ParserError::NullCharacterReference => "null-character-reference",
            ParserError::SurrogateCharacterReference => "surrogate-character-reference",
            ParserError::SurrogateInInputStream => "surrogate-in-input-stream",
            ParserError::UnexpectedCharacterAfterDoctypeSystemIdentifier => "unexpected-character-after-doctype-system-identifier",
            ParserError::UnexpectedCharacterInAttributeName => "unexpected-character-in-attribute-name",
            ParserError::UnexpectedCharacterInUnquotedAttributeValue => "unexpected-character-in-unquoted-attribute-value",
            ParserError::UnexpectedEqualsSignBeforeAttributeName => "unexpected-equals-sign-before-attribute-name",
            ParserError::UnexpectedNullCharacter => "unexpected-null-character",
            ParserError::UnexpectedQuestionMarkInsteadOfTagName => "unexpected-question-mark-instead-of-tag-name",
            ParserError::UnexpectedSolidusInTag => "unexpected-solidus-in-tag",
            ParserError::UnknownNamedCharacterReference => "unknown-named-character-reference",
            ParserError::AbruptClosingOfEmptyComment => "abrupt-closing-of-empty-comment",
        }
    }
}
