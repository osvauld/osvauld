# Navigation Button Guide

## Overview

The `nav-button` component is a unified button that can handle **3 different modes**:

1. **MODE 1**: Simple navigation (show/hide/toggle containers or navigate screens)
2. **MODE 2**: Submit entire form + navigate (replaces `form-submit-button`)
3. **MODE 3**: Set field value + navigate (replaces `branching-question`)

This consolidates functionality and eliminates the need for separate `form-submit-button` and `branching-question` components.

---

## MODE 1: Simple Navigation

Just navigate between screens or show/hide containers.

### Properties
- `type: "nav-button"`
- `content`: Button text
- `action`: `"navigate"`, `"show"`, `"hide"`, or `"toggle"`
- `targetContainerId`: Target screen/container ID
- `css`: Styling (optional)

### Example
```yaml
- ::
  type: "nav-button"
  content: "Go to Next Page"
  action: "navigate"
  targetContainerId: "screen-2"
  css: "padding: 1rem 2rem; background: #89b4fa; color: #1e1e2e;"
```

---

## MODE 2: Submit Entire Form + Navigate

Submit all form fields and navigate to a thank-you page (replaces `form-submit-button`).

### Properties
- `type: "nav-button"`
- `content`: Button text
- `formId`: ID of the form to submit (e.g., `"form-registration"`)
- `action: "navigate"`
- `targetContainerId`: Where to navigate after submission
- `css`: Styling (optional)

### How It Works
1. Collects **all** form fields with matching `formId`
2. Validates required fields
3. Submits data to `submissionsDoc`
4. Navigates to target screen

### Example
```yaml
# Form definition
- ::
  type: "form"
  name: "Registration Form"
  eventName: "user_registration"

# Form fields
- ::
  type: "form-field-text"
  formId: "form-registration"
  fieldName: "email"
  label: "Email"
  required: true

# Submit button (MODE 2)
- ::
  type: "nav-button"
  formId: "form-registration"
  content: "Submit Registration"
  action: "navigate"
  targetContainerId: "thank-you"
  css: "background: #a6e3a1; padding: 1rem 2rem;"
```

---

## MODE 3: Set Field Value + Navigate (Branching)

Set a specific form field to a value and navigate (replaces `branching-question`).

### Properties
- `type: "nav-button"`
- `content`: Button text (what the user sees)
- `formId`: ID of the form to submit
- `fieldName`: The field name to set (e.g., `"customer_type"`)
- `value`: The value to assign to this field (e.g., `"new_customer"`)
- `action: "navigate"`
- `targetContainerId`: Where to navigate after submission
- `css`: Styling (optional)

### How It Works
1. Collects any **existing** form fields (if any)
2. Sets `formData[fieldName] = value`
3. Submits data to `submissionsDoc`
4. Navigates to target screen

### Example: Branching Question
```yaml
# Form to track user journey
- ::
  type: "form"
  name: "Customer Journey"
  eventName: "customer_type_selection"

# Button 1: New Customer (MODE 3)
- ::
  type: "nav-button"
  formId: "form-customer-journey"
  fieldName: "customer_type"
  value: "new_customer"
  content: "Yes, I'm a New Customer"
  action: "navigate"
  targetContainerId: "new-customer-flow"
  css: "background: #a6e3a1; padding: 1.5rem;"

# Button 2: Returning Customer (MODE 3)
- ::
  type: "nav-button"
  formId: "form-customer-journey"
  fieldName: "customer_type"
  value: "returning_customer"
  content: "No, I'm Returning"
  action: "navigate"
  targetContainerId: "returning-customer-flow"
  css: "background: #f9e2af; padding: 1.5rem;"
```

### Submission Data
When "Yes, I'm a New Customer" is clicked:
```json
{
  "customer_type": "new_customer"
}
```

---

## Complete Example: Multi-Step Branching Form

See `templates/branching-form-demo.huml` for a full working example that demonstrates:

1. **Initial branching question** (MODE 3)
   - New customer → Registration form
   - Returning customer → Issue type selection

2. **Second branching question** (MODE 3)
   - Technical issue → Tech support form
   - Billing question → Billing form

3. **Form submissions** (MODE 2)
   - Registration form → Thank you page
   - Support forms → Thank you page

---

## Migration Guide

### From `form-submit-button` → `nav-button` (MODE 2)

**Before:**
```yaml
- ::
  type: "form-submit-button"
  formId: "form-123"
  content: "Submit"
  targetContainerId: "thank-you"
```

**After:**
```yaml
- ::
  type: "nav-button"
  formId: "form-123"
  content: "Submit"
  action: "navigate"
  targetContainerId: "thank-you"
```

### From `branching-question` → `nav-button` (MODE 3)

**Before:**
```yaml
- ::
  type: "branching-question"
  question: "Are you a new customer?"
  yesLabel: "Yes"
  noLabel: "No"

- ::
  type: "nav-button"
  content: "Continue"
  yesTargetId: "new-customer"
  noTargetId: "returning-customer"
```

**After:**
```yaml
# Form to track choice
- ::
  type: "form"
  name: "Customer Type"
  eventName: "customer_selection"

# Yes button
- ::
  type: "nav-button"
  formId: "form-customer-type"
  fieldName: "customer_type"
  value: "new"
  content: "Yes"
  action: "navigate"
  targetContainerId: "new-customer"

# No button
- ::
  type: "nav-button"
  formId: "form-customer-type"
  fieldName: "customer_type"
  value: "returning"
  content: "No"
  action: "navigate"
  targetContainerId: "returning-customer"
```

---

## Benefits of Unified Approach

1. **Fewer components** to learn
2. **More flexible** - can have N options, not just yes/no
3. **Consistent API** - same properties across all modes
4. **Recorded choices** - branching decisions are saved as form data
5. **Composable** - can combine field values with other form fields

---

## Properties Panel Configuration

In the Properties Panel, when you select a nav-button:

1. **Form (Optional)**: Select a form to enable MODE 2 or MODE 3
2. **Field Name (Optional)**: Enter a field name to enable MODE 3 (branching mode)
3. **Field Value**: Enter the value this button will set (only if Field Name is set)

The panel shows helpful hints about which mode is active based on what you've configured.
