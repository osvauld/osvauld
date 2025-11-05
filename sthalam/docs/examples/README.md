# HUML Template Examples

Working examples demonstrating HUML features. All examples are fully tested and functional.

## Examples

### 00_simple_counter.huml
**Demonstrates:**
- Basic state definition (`publisherState`)
- Computed values (`publisherComputed`)
- Button actions with `setState`
- String interpolation in content
- Conditional rendering with `when`
- Container layouts with flexbox

**Concepts:**
- State updates via button clicks
- Reactive computed values
- Basic styling with inline CSS

---

### 01_cel_functions.huml
**Demonstrates:**
- Built-in CEL functions: `timestamp()`, `generateId()`
- Button actions that capture dynamic values
- Multiple sections with different backgrounds
- Live updates (timestamp/ID regenerate on each render)

**Concepts:**
- Using CEL functions in state updates
- Displaying dynamic values in text blocks
- Visual feedback with conditional rendering

---

### 02_form_blocks.huml
**Demonstrates:**
- Text input fields (`input`)
- Multi-line text input (`textarea`)
- Checkboxes with labels (`checkbox`)
- Form labels (`label`)
- Dropdown selects (`select`)
- Radio button groups (`radio`)
- Email validation with computed values
- Character counting with `size()` function
- Submit button with validation

**Concepts:**
- Local state binding for all form inputs
- Reactive computed values for validation
- Multiple input types (text, email, checkbox, select, radio)
- Form validation and disabled states
- Clear all functionality

---

### 03_content_blocks.huml
**Demonstrates:**
- External links with static `href` URLs (`link`)
- SPA navigation links with `action="navigate"` (`link`)
- Static and dynamic images (`image`)
- Image size controls with state
- Conditional image display with `when`
- Grid layouts with multiple images
- Multi-screen navigation

**Concepts:**
- External links open in system browser (security model)
- Link content supports interpolation (display text only)
- External URLs must be static strings (no dynamic URLs)
- Navigation between screens using action/params
- Dynamic image attributes (src, width, height)
- Image controls with button state updates
- Grid layout for image galleries
- Two-screen app structure

---

### 04_form_submissions.huml
**Demonstrates:**
- Complete form container (`form`)
- Form submission handling
- Persistent storage in `submissionsDoc` (Loro CRDT)
- Multiple forms in single template
- Submit buttons with `submitForm: true`
- Form validation with computed values
- Character counters
- Conditional form visibility with toggle
- All form input types in context

**Concepts:**
- Forms automatically collect field values from context
- Submissions stored persistently in submissionsDoc
- Each submission includes: ID, timestamp, form name, field data
- Submit buttons trigger form's onsubmit event
- Form validation with disabled state
- Viewing submissions in SubmissionsViewer component
- Export submissions as CSV or JSON
- Filter submissions by form name (event name)
- Submissions persist across sessions

**How it works:**
1. Form wraps input fields in `<form>` element
2. Submit button with `submitForm: true` becomes `type="submit"`
3. On form submission, FormBlock collects all field values from context
4. Submission is saved to `submissionsDoc` via SubmissionsStore
5. View all submissions in the Submissions tab
6. Filter, export, and analyze form data

---

## How to Use

1. **Import into Sthalam:**
   - Copy the HUML content
   - In Sthalam, create a new website resource
   - Paste the HUML code
   - Preview in Publisher mode

2. **Learn by Modification:**
   - Change state initial values
   - Modify computed expressions
   - Adjust styling via `css` properties
   - Add new blocks

3. **Reference for LLMs:**
   - Use these as working examples when generating templates
   - Copy patterns for common features
   - See HUML v0.1.0 syntax in action

---

## Template Structure

All templates follow this structure:

```yaml
name: "Template Name"
version: "v1.0.0"

documents::
  publisherState::
    # State field definitions

  publisherComputed::
    # Computed value expressions

ui::
  publisher::
    - ::
      type: "screen"
      # Screen blocks
```

---

## Next Steps

See `/docs/specs/HUML_TEMPLATE_GUIDE_ACCURATE.md` for:
- Complete syntax reference
- All block types and properties
- CEL expression guide
- Implementation status
