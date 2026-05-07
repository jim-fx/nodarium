# Comprehensive UX Practices for Web Applications

## Introduction

This document consolidates many of the most important practical UX principles for modern web applications.

---

# 1. Core UX Principles

## 1.1 Visibility of System Status

Users should always understand:

* What the system is doing
* Whether an action succeeded
* Whether work is still in progress
* Whether an error occurred

### Good Practices

* Show loading indicators immediately
* Show success confirmations after important actions
* Show inline validation messages
* Display progress for long-running tasks
* Use skeleton loading states instead of blank screens
* Prevent silent failures
* Avoid ambiguous UI states

### Bad Practices

* Buttons with no feedback after clicking
* Infinite spinners without explanation
* Hidden background operations
* Saving without visible confirmation

---

## 1.2 Predictability and Consistency

Users build mental models quickly.

Breaking established expectations increases cognitive load and causes mistakes.

### Good Practices

* Use consistent layouts
* Keep interaction patterns stable
* Reuse common UI conventions
* Keep naming and terminology consistent
* Use standard keyboard shortcuts
* Make similar components behave similarly

### Bad Practices

* Different button styles for identical actions
* Inconsistent navigation behavior
* Custom controls that ignore platform conventions
* Unexpected modal behavior

---

## 1.3 Recognition Over Recall

Interfaces should minimize memory requirements.

Users should recognize options instead of remembering information.

### Good Practices

* Show recent searches
* Use autocomplete
* Display contextual hints
* Preserve previously entered values
* Use visible labels
* Keep important actions visible

### Bad Practices

* Placeholder-only labels
* Hidden functionality
* Requiring users to remember previous state
* Removing useful context during workflows

---

## 1.4 Error Prevention

Preventing mistakes is better than handling mistakes.

### Good Practices

* Disable impossible actions
* Validate input early
* Warn before destructive operations
* Use constrained input formats
* Use safe defaults
* Prefer undo over confirmation dialogs

### Bad Practices

* Destructive actions near common actions
* Easy accidental deletion
* Poor validation timing
* Irreversible operations without recovery

---

# 2. Input and Form UX

Forms are one of the most important and failure-prone areas in web applications.

---

## 2.1 Input Focus Behavior

### Good Practices

* Autofocus the primary field when appropriate
* Preserve focus during rerenders
* Preserve cursor position
* Support keyboard-first workflows
* Use logical tab ordering

### Auto-Selecting Input Text

Auto-selecting text on focus is context-dependent.

### Good Use Cases

* Quantity fields
* Rename dialogs
* Editable defaults
* Quick replacement workflows
* Temporary values users often replace entirely

### Bad Use Cases

* Long textareas
* Complex text editing
* Fields users commonly partially edit
* Rich text editing

### Principle

Only auto-select when full replacement is more likely than partial editing.

---

## 2.2 Labels and Placeholders

### Good Practices

* Always use visible labels
* Use placeholders only as supplementary examples
* Keep labels visible after typing
* Associate labels correctly for accessibility

### Bad Practices

* Placeholder-only forms
* Ambiguous labels
* Labels that disappear during editing

---

## 2.3 Validation

### Recommended Validation Timing

| Validation Type     | Timing    |
| ------------------- | --------- |
| Format validation   | Immediate |
| Semantic validation | On blur   |
| Server validation   | On submit |

### Good Practices

* Show errors near the relevant field
* Explain how to fix issues
* Preserve entered values after errors
* Validate incrementally
* Use clear language

### Bad Practices

* Generic “Invalid input” messages
* Clearing form data after errors
* Delayed validation surprises
* Validation that interrupts typing

---

## 2.4 Input Types

### Good Practices

Use appropriate HTML input types:

* `email`
* `tel`
* `number`
* `date`
* `password`
* `search`

### Benefits

* Better mobile keyboards
* Native validation
* Improved accessibility
* Better autofill support

---

## 2.5 Form Submission

### Good Practices

* Enter submits forms when expected
* Escape cancels dialogs
* Show loading states during submission
* Prevent duplicate submissions
* Preserve draft state
* Allow keyboard submission

### Bad Practices

* Disabled submit buttons without explanation
* Hidden validation failures
* Silent submission failures

---

## 2.6 Dropdowns and Selection UX

### Good Practices

* Use radio buttons for small option sets
* Use searchable selects for large datasets
* Prefer autocomplete for many options
* Show selected state clearly

### Bad Practices

* Massive unsearchable dropdowns
* Nested dropdown hierarchies
* Multi-select controls without search

---

# 3. Navigation UX

---

## 3.1 Orientation

Users should always know:

* Where they are
* How they got there
* What they can do next
* How to go back

### Good Practices

* Highlight active navigation
* Use breadcrumbs when helpful
* Use meaningful page titles
* Preserve navigation consistency

---

## 3.2 Navigation Structure

### Good Practices

* Keep hierarchy shallow
* Group related actions
* Use descriptive names
* Keep primary actions stable

### Bad Practices

* Deep nesting
* Ambiguous navigation labels
* Constantly moving actions

---

## 3.3 URL Design

### Good Practices

* Use readable URLs
* Make URLs shareable
* Preserve app state in URLs when useful
* Support browser history correctly

### Bad Practices

* Opaque generated URLs
* Broken back button behavior
* Losing state during navigation

---

# 4. Interaction Design

---

## 4.1 Click Targets

### Good Practices

* Large clickable areas
* Adequate spacing between actions
* Clear hover/focus states
* Touch-friendly sizing

### Bad Practices

* Tiny clickable regions
* Overlapping interactive elements
* Hidden hit areas

---

## 4.2 Feedback

Every interaction should produce feedback.

### Good Practices

* Hover states
* Active states
* Loading indicators
* Success states
* Error states
* Optimistic updates when appropriate

### Bad Practices

* Dead-feeling interfaces
* Invisible processing
* Delayed reactions

---

## 4.3 Destructive Actions

### Good Practices

* Require confirmation for dangerous actions
* Prefer undo systems
* Visually distinguish destructive buttons
* Separate destructive actions spatially

### Bad Practices

* Immediate irreversible deletion
* Dangerous actions near common actions
* Ambiguous destructive wording

---

## 4.4 Modal UX

### Good Practices

* Trap keyboard focus
* Support Escape to close
* Restore focus after closing
* Prevent background interaction
* Keep modal purpose focused

### Bad Practices

* Nested modals
* Full workflows inside modals
* Losing unsaved work accidentally

---

# 5. Performance UX

Performance is a UX feature.

Users interpret slowness as unreliability.

---

## 5.1 Perceived Performance

### Good Practices

* Show immediate visual response
* Use optimistic UI updates
* Preload likely next content
* Stream content progressively
* Use skeleton loaders

### Bad Practices

* Blank screens during loading
* Long blocking operations
* Frozen interfaces

---

## 5.2 Layout Stability

### Good Practices

* Prevent layout shift
* Reserve image dimensions
* Avoid moving buttons during loading
* Keep skeletons aligned with final layout

### Bad Practices

* Jumping content
* Shifting controls
* Reflow-heavy rendering

---

## 5.3 Responsiveness

### Good Practices

* Keep UI interactive during async operations
* Avoid blocking the main thread
* Debounce expensive operations
* Virtualize large lists

### Bad Practices

* UI freezes
* Excessive rerenders
* Laggy typing experiences

---

# 6. Accessibility

Accessibility improves usability for everyone.

---

## 6.1 Keyboard Accessibility

### Good Practices

* Full keyboard navigation
* Visible focus indicators
* Logical tab order
* Keyboard shortcuts for power users

### Bad Practices

* Mouse-only workflows
* Hidden focus state
* Keyboard traps

---

## 6.2 Semantic HTML

### Good Practices

* Use proper semantic elements
* Use buttons for actions
* Use links for navigation
* Use headings correctly

### Bad Practices

* Clickable divs without accessibility support
* Fake buttons
* Missing semantic structure

---

## 6.3 Visual Accessibility

### Good Practices

* Sufficient color contrast
* Support reduced motion
* Avoid color-only communication
* Use scalable typography

### Bad Practices

* Tiny text
* Low contrast interfaces
* Flashing animations

---

## 6.4 Screen Reader Support

### Good Practices

* Proper labels
* Meaningful alt text
* ARIA only when necessary
* Correct live regions for updates

### Bad Practices

* Unlabeled controls
* Excessive ARIA misuse
* Non-announced state changes

---

# 7. Enterprise Application UX

Enterprise UX differs significantly from marketing-oriented consumer interfaces.

Power users often prioritize efficiency over visual minimalism.

---

## 7.1 Dense Information Design

### Good Practices

* Efficient data density
* Resizable tables
* Sticky headers
* Multi-column layouts
* High information throughput

### Bad Practices

* Excessive whitespace
* Oversimplified dashboards
* Hidden operational controls

---

## 7.2 Table UX

### Good Practices

* Sorting
* Filtering
* Column resizing
* Pagination or virtualization
* Keyboard navigation
* Export functionality
* Persistent user preferences

### Bad Practices

* Non-sortable enterprise tables
* Horizontal scrolling nightmares
* Missing filtering

---

## 7.3 Power User Workflows

### Good Practices

* Keyboard shortcuts
* Bulk actions
* Batch editing
* Command palettes
* State persistence
* Fast navigation

### Bad Practices

* Forced wizard workflows
* Excessive confirmations
* Repetitive manual work

---

# 8. Mobile UX

---

## 8.1 Touch Design

### Good Practices

* Large touch targets
* Thumb-friendly layouts
* Avoid hover dependencies
* Mobile-friendly spacing

### Bad Practices

* Tiny controls
* Hover-only interactions
* Precision-dependent gestures

---

## 8.2 Mobile Forms

### Good Practices

* Mobile keyboard optimization
* Minimal typing
* Autofill support
* Step-by-step flows when necessary

### Bad Practices

* Long complex forms
* Tiny input fields
* Excessive required typing

---

# 9. Cognitive Psychology and UX

---

## 9.1 Hick’s Law

More choices increase decision time.

### Applications

* Reduce unnecessary options
* Group related actions
* Prioritize primary actions

---

## 9.2 Fitts’s Law

Closer and larger targets are easier to use.

### Applications

* Large primary buttons
* Edge/corner placement for important actions
