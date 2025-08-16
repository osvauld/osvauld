import { Schema, type NodeSpec, type MarkSpec } from "prosemirror-model";
import { schema as basicSchema } from "prosemirror-schema-basic";
import { addListNodes } from "prosemirror-schema-list";
import { tableNodes } from "prosemirror-tables";
/**
 * Custom node specifications
 */
const imageSpec: NodeSpec = {
  inline: true,
  attrs: {
    src: {},
    alt: { default: null },
    title: { default: null },
    width: { default: null },
    height: { default: null },
    margin: { default: 12 }
  },
  group: "inline",
  draggable: true,
  parseDOM: [{
    tag: "img[src]",
    getAttrs(dom: HTMLElement) {
      const marginAttr = dom.getAttribute("data-margin");
      const margin = marginAttr != null ? parseInt(marginAttr, 10) : 12;
      return {
        src: dom.getAttribute("src"),
        alt: dom.getAttribute("alt"),
        title: dom.getAttribute("title"),
        width: dom.getAttribute("width"),
        height: dom.getAttribute("height"),
        margin
      };
    }
  }],
  toDOM(node) {
    const { margin, ...htmlAttrs } = node.attrs;
    const finalMargin = margin ?? 12;

    return ["img", {
      ...htmlAttrs,
      style: `margin: ${finalMargin}px`,
      "data-margin": finalMargin
    }];
  }
};
/**
 * Helper to add indent and align attributes to a node spec
 */
function addIndentAndAlignAttrs(nodeSpec: NodeSpec): NodeSpec {
  return {
    ...nodeSpec,
    attrs: {
      ...nodeSpec.attrs,
      align: { default: null },
      indent: { default: null },
    },
    parseDOM: [
      {
        tag: nodeSpec.parseDOM?.[0]?.tag || "p",
        getAttrs(dom: HTMLElement) {
          const existingAttrs = nodeSpec.parseDOM?.[0]?.getAttrs ?
            nodeSpec.parseDOM[0].getAttrs(dom) :
            {};
          return {
            ...existingAttrs,
            align: dom.style.textAlign || null,
            indent: dom.hasAttribute("data-indent")
              ? parseInt(dom.getAttribute("data-indent") || "0", 10)
              : null,
          };
        },
      },
    ],
    toDOM(node) {
      const attrs: { [key: string]: any } = {};

      if (node.attrs.align) {
        attrs.style = `text-align: ${node.attrs.align}`;
      }

      if (node.attrs.indent && node.attrs.indent > 0) {
        attrs["data-indent"] = node.attrs.indent;
      }

      return [nodeSpec.parseDOM?.[0]?.tag || "p", attrs, 0] as [string, Object, number];
    },
  };
}

/**
 * Custom mark specifications
 */
const customMarks: { [key: string]: MarkSpec } = {
  strong: {
    parseDOM: [
      { tag: "strong" },
      { tag: "b" },
      {
        tag: "span",
        getAttrs: (node: HTMLElement) => node.style.fontWeight != "normal" && null,
      },
    ],
    toDOM() {
      return ["strong", 0];
    },
  },
  em: {
    parseDOM: [{ tag: "i" }, { tag: "em" }, { style: "font-style=italic" }],
    toDOM() {
      return ["em", 0];
    },
  },
  code: {
    parseDOM: [{ tag: "code" }],
    toDOM() {
      return ["code", 0];
    },
  },
  fontSize: {
    attrs: {
      size: { default: null }
    },
    inclusive: true,
    parseDOM: [{
      style: "font-size",
      getAttrs: (value) => value ? { size: value } : null
    }],
    toDOM(mark) {
      return mark.attrs.size
        ? ["span", { style: `font-size: ${mark.attrs.size}` }, 0]
        : ["span", 0];
    }
  },
  underline: {
    parseDOM: [
      { tag: "u" },
      { style: "text-decoration=underline" }
    ],
    toDOM() {
      return ["u", 0];
    },
  },
  strikethrough: {
    parseDOM: [
      { tag: "s" },
      { tag: "strike" },
      { tag: "del" },
      { style: "text-decoration=line-through" }
    ],
    toDOM() {
      return ["s", 0];
    }
  },
  textColor: {
    attrs: {
      color: { default: null }
    },
    inclusive: true,
    parseDOM: [{
      style: "color",
      getAttrs: (value) => value ? { color: value } : null
    }],
    toDOM(mark) {
      return mark.attrs.color
        ? ["span", { style: `color: ${mark.attrs.color}` }, 0]
        : ["span", 0];
    }
  },
  fontFamily: {
    attrs: {
      family: { default: null }
    },
    inclusive: true,
    parseDOM: [{
      style: "font-family",
      getAttrs: (value) => value ? { family: value } : null
    }],
    toDOM(mark) {
      return mark.attrs.family
        ? ["span", { style: `font-family: ${mark.attrs.family}` }, 0]
        : ["span", 0];
    }
  },
  link: {
    attrs: {
      href: { default: null },
      title: { default: null }
    },
    inclusive: false,
    excludes: "underline",
    parseDOM: [{
      tag: "a[href]",
      getAttrs(dom: HTMLElement) {
        const href = dom.getAttribute("href");
        const dataMceHref = dom.getAttribute("data-mce-href");
        let finalHref = href;

        if (!href || href.trim() === "" || href.trim() === "#") {
          if (dataMceHref && dataMceHref.trim() !== "") {
            finalHref = dataMceHref;
          }
        }

        if (!finalHref || finalHref.trim() === "") {
          return false;
        }

        return {
          href: finalHref,
          title: dom.getAttribute("title") || dom.textContent?.trim() || "",
        };
      },
    }],
    toDOM(mark) {
      return ["a", {
        href: mark.attrs.href,
        title: mark.attrs.title,
        target: "_blank",
        rel: "noopener noreferrer"
      }, 0];
    }
  },
  comment: {
    attrs: {
      threadId: {},
      commentIds: { default: [] },
      resolved: { default: false },
      author: { default: null }
    },
    inclusive: false,
    excludes: "",
    parseDOM: [{
      tag: "span[data-livnote-comment]",
      getAttrs(dom: HTMLElement) {
        return {
          threadId: dom.getAttribute("data-livnote-comment"),
          commentIds: JSON.parse(dom.getAttribute("data-livnote-comment-ids") || "[]"),
          resolved: dom.getAttribute("data-livnote-resolved") === "true",
          author: dom.getAttribute("data-livnote-author") || null
        };
      }
    }],
    toDOM(mark) {
      const { threadId, commentIds, resolved, author } = mark.attrs;
      return ["span", {
        "data-livnote-comment": threadId,
        "data-livnote-comment-ids": JSON.stringify(commentIds),
        "data-livnote-comment-count": commentIds.length.toString(),
        "data-livnote-resolved": resolved ? "true" : "false",
        "data-livnote-author": author || "",
        "data-livnote-internal": "true",
        class: `livnote-comment-highlight ${resolved ? 'resolved' : 'active'}`,
        style: resolved
          ? "border-bottom: 2px solid #888; background: rgba(136, 136, 136, 0.1);"
          : "border-bottom: 2px solid #ffd700; background: rgba(255, 215, 0, 0.1);"
      }, 0];
    }
  }
};

/**
 * Create the editor schema with all custom nodes and marks
 */
export function createEditorSchema(): Schema {
  const nodes = basicSchema.spec.nodes;

  // Get base node specs
  const paragraphSpec = nodes.get("paragraph");
  const headingSpec = nodes.get("heading");

  if (!paragraphSpec || !headingSpec) {
    throw new Error("Base schema missing required nodes");
  }

  // Modify heading spec with alignment and indent
  const modifiedHeadingSpec: NodeSpec = {
    ...headingSpec,
    attrs: {
      ...headingSpec.attrs,
      align: { default: null },
      indent: { default: null },
    },
    parseDOM: (headingSpec.parseDOM || []).map(spec => ({
      tag: spec.tag,
      getAttrs(dom: HTMLElement) {
        const existingAttrs = spec.getAttrs ? spec.getAttrs(dom) : {};
        return {
          ...existingAttrs,
          align: dom.style.textAlign || null,
          indent: dom.hasAttribute("data-indent")
            ? parseInt(dom.getAttribute("data-indent") || "0", 10)
            : null,
        };
      }
    })),
    toDOM(node) {
      const attrs: { [key: string]: any } = {};

      if (node.attrs.align) {
        attrs.style = `text-align: ${node.attrs.align}`;
      }

      if (node.attrs.indent && node.attrs.indent > 0) {
        attrs["data-indent"] = node.attrs.indent;
      }

      return [`h${node.attrs.level}`, attrs, 0] as [string, Object, number];
    }
  };

  // Update nodes with modifications
  const modifiedNodes = nodes
    .update("paragraph", addIndentAndAlignAttrs(paragraphSpec))
    .update("heading", modifiedHeadingSpec);

  // Add list nodes and image node
  const nodesWithListsAndImage = addListNodes(modifiedNodes, "paragraph block*", "block")
    .addToEnd("image", imageSpec);

  // Add table nodes - this is the new part!
  const tableNodeSpecs = tableNodes({
    tableGroup: "block",
    cellContent: "block+", // Allows rich content (paragraphs, headings, lists, etc.)
    cellAttributes: {
      background: {
        default: null,
        getFromDOM(dom: HTMLElement) {
          return dom.style.backgroundColor || null;
        },
        setDOMAttr(value: string | null, attrs: any) {
          if (value) attrs.style = (attrs.style || "") + `background-color: ${value};`;
        }
      }
    }
  });

  // Add table nodes to the schema
  const finalNodes = nodesWithListsAndImage
    .addToEnd("table", tableNodeSpecs.table)
    .addToEnd("table_row", tableNodeSpecs.table_row)
    .addToEnd("table_cell", tableNodeSpecs.table_cell)
    .addToEnd("table_header", tableNodeSpecs.table_header);

  return new Schema({
    nodes: finalNodes,
    marks: customMarks
  });
}
/**
 * Get mark by name from schema
 */
export function getMark(schema: Schema, markName: string) {
  return schema.marks[markName];
}

/**
 * Get node by name from schema
 */
export function getNode(schema: Schema, nodeName: string) {
  return schema.nodes[nodeName];
}
