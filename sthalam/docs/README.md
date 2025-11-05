# Sthalam Documentation

This directory contains all Sthalam project documentation, organized by category.

## 📁 Directory Structure

### `/architecture/`
Core architectural design documents and system design decisions.

- **STHALAM_ARCHITECTURE_GUIDE.md** - Overall system architecture, components, and data flow
- **NEW_BLOCK_RENDERER_DESIGN.md** - BlockRenderer redesign rationale and implementation plan

### `/guides/`
Implementation guides, tutorials, and how-to documents.

- **IMPLEMENTATION_COMPLETE.md** - CEL evaluator implementation completion guide
- **HUML_PARSER_COMPLETE.md** - HUML parser implementation and integration guide
- **INTEGRATION_GUIDE.md** - How to integrate the new WASM-based services
- **LORO_SIMPLIFICATION.md** - Migration from Tree-based to Map-based Loro architecture
- **WASM_TAURI_KNOWLEDGE.md** - WASM performance optimizations and Tauri integration

### `/specs/`
Technical specifications for block types, languages, and APIs.

- **STHALAM_DSL.md** - HUML template language specification
- **BLOCK_RENDERER_SPEC.md** - Complete block renderer implementation specification
- **CANVAS_BLOCK_SPEC.md** - Canvas block WebGL rendering specification
- **HUML_TEMPLATE_GUIDE_ACCURATE.md** - ✅ **ACCURATE** template guide reflecting current implementation

### `/reference/`
API references and technical reference material.

- *(Currently empty - to be populated with API docs)*

### `/archive/`
Outdated or superseded documentation kept for historical reference.

- **HUML_TEMPLATE_GUIDE_V3.md** - ⚠️ OUTDATED: Claims features not yet implemented
- **OCAML_MIGRATION_PLAN.md** - Historical: OCaml/WASM migration planning
- **IMPLEMENTATION_ROADMAP.md** - Historical: Original implementation roadmap
- **CEL_IMPLEMENTATION_STATUS.md** - Historical: CEL implementation progress tracking

---

## 🚀 Quick Start

**New to Sthalam?** Read these in order:

1. **[Architecture Guide](architecture/STHALAM_ARCHITECTURE_GUIDE.md)** - Understand the system
2. **[HUML Template Guide](specs/HUML_TEMPLATE_GUIDE_ACCURATE.md)** - Learn template syntax (✅ accurate!)
3. **[Integration Guide](guides/INTEGRATION_GUIDE.md)** - Set up your development environment

---

## ⚠️ Documentation Status

### ✅ ACCURATE & CURRENT
- HUML_TEMPLATE_GUIDE_ACCURATE.md - **Use this for templates**
- IMPLEMENTATION_COMPLETE.md
- HUML_PARSER_COMPLETE.md
- WASM_TAURI_KNOWLEDGE.md
- LORO_SIMPLIFICATION.md

### ⚠️ OUTDATED (Archived)
- HUML_TEMPLATE_GUIDE_V3.md - Claims unimplemented features
- IMPLEMENTATION_ROADMAP.md - Planning document, partially outdated
- CEL_IMPLEMENTATION_STATUS.md - Historical status, now complete

### 📋 NEEDS UPDATE
- BLOCK_RENDERER_SPEC.md - Lists 20+ block types, only 6 fully implemented
- CANVAS_BLOCK_SPEC.md - Canvas is currently a stub
- STHALAM_DSL.md - Some block types not yet implemented

---

## 🎯 Current Implementation Status (2025-01-04)

### ✅ FULLY WORKING
- **CEL Evaluator (WASM)** - OCaml-compiled, 40x faster than JS
- **HUML Parser (WASM)** - Parses HUML templates to JavaScript objects
- **BlockRenderer** - Full control flow: `when`, `if/else`, `match`, `forEach`
- **Core Blocks (6)** - Text, Heading, Button, Container, Section, Screen
- **Actions (2)** - `setState` and `navigate`
- **Loro CRDT** - Simplified Map-based architecture

### 🚧 STUB/TODO (12 blocks)
Input, Textarea, Checkbox, Select, Radio, Label, Link, Form, Image, Video, Canvas, Modal

**Implementation Rate:** 33% (6/18 blocks functional)

---

## 📊 Performance Achievements

- **CEL Evaluation:** 60 FPS for 10,000 expressions/frame
- **HUML Parsing:** 10KB template in ~15ms
- **WASM Loading:** < 100ms cold start
- **Zero-copy GPU upload:** Direct Uint8Array → WebGL

---

## 🔧 Development

### Building Documentation
Documentation is written in Markdown and can be viewed in any Markdown viewer.

### Adding New Docs
- Architecture decisions → `/architecture/`
- How-to guides → `/guides/`
- API specs → `/specs/`
- Reference material → `/reference/`

### Updating Outdated Docs
Move outdated docs to `/archive/` and create updated versions in the appropriate directory.

---

## 📝 Contributing

When updating documentation:
1. ✅ **Validate claims against actual code**
2. ✅ **Mark implementation status** (✅ working, 🚧 stub, ❌ not implemented)
3. ✅ **Include code examples** from actual codebase
4. ✅ **Update this README** if adding/removing major docs

---

## 📧 Questions?

For questions about documentation or clarification, check:
1. `/guides/INTEGRATION_GUIDE.md` - Integration help
2. `/specs/HUML_TEMPLATE_GUIDE_ACCURATE.md` - Template syntax
3. `/guides/WASM_TAURI_KNOWLEDGE.md` - Performance and WASM

---

**Last Updated:** 2025-01-04
**Sthalam Version:** v0.1.0
