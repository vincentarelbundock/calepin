#let _input-syntax-theme = bytes((
  "<?xml version=\"1.0\" encoding=\"UTF-8\"?>",
  "<!DOCTYPE plist PUBLIC \"-//Apple//DTD PLIST 1.0//EN\" \"http://www.apple.com/DTDs/PropertyList-1.0.dtd\">",
  "<plist version=\"1.0\">",
  "<dict>",
  "  <key>name</key>",
  "  <string>Calepin HTML Syntax Sentinel</string>",
  "  <key>settings</key>",
  "  <array>",
  "    <dict>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#000000</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>Basic text &amp; variable names (incl. leading punctuation)</string>",
  "      <key>scope</key>",
  "      <string>text, source, variable.other.readwrite, punctuation.definition.variable</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#000001</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>Parentheses, Brackets, Braces</string>",
  "      <key>scope</key>",
  "      <string>punctuation</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#000002</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>Comments</string>",
  "      <key>scope</key>",
  "      <string>comment, punctuation.definition.comment</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#000003</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>string, punctuation.definition.string</string>",
  "      <key>scope</key>",
  "      <string>string, punctuation.definition.string</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#000004</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>constant.character.escape</string>",
  "      <key>scope</key>",
  "      <string>constant.character.escape</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#000005</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>Booleans, constants, numbers</string>",
  "      <key>scope</key>",
  "      <string>constant.numeric, variable.other.constant, entity.name.constant, constant.language.boolean, constant.language.false, constant.language.true, keyword.other.unit.user-defined, keyword.other.unit.suffix.floating-point</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#000006</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>keyword, keyword.operator.word, keyword.operator.new, variable.language.super, support.type.primitive, storage.type, storage.modifier, punctuation.definition.keyword</string>",
  "      <key>scope</key>",
  "      <string>keyword, keyword.operator.word, keyword.operator.new, variable.language.super, support.type.primitive, storage.type, storage.modifier, punctuation.definition.keyword</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#000007</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>entity.name.tag.documentation</string>",
  "      <key>scope</key>",
  "      <string>entity.name.tag.documentation</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#000008</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>Punctuation</string>",
  "      <key>scope</key>",
  "      <string>keyword.operator, punctuation.accessor, punctuation.definition.generic, meta.function.closure punctuation.section.parameters, punctuation.definition.tag, punctuation.separator.key-value</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#000009</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>entity.name.function, meta.function-call.method, support.function, support.function.misc, variable.function</string>",
  "      <key>scope</key>",
  "      <string>entity.name.function, meta.function-call.method, support.function, support.function.misc, variable.function</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#00000a</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>Classes</string>",
  "      <key>scope</key>",
  "      <string>entity.name.class, entity.other.inherited-class, support.class, meta.function-call.constructor, entity.name.struct</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#00000b</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>Enum</string>",
  "      <key>scope</key>",
  "      <string>entity.name.enum</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#00000c</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>Enum member</string>",
  "      <key>scope</key>",
  "      <string>meta.enum variable.other.readwrite, variable.other.enummember</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#00000d</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>Object properties</string>",
  "      <key>scope</key>",
  "      <string>meta.property.object</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#00000e</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>Types</string>",
  "      <key>scope</key>",
  "      <string>meta.type, meta.type-alias, support.type, entity.name.type</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#00000f</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>Decorators</string>",
  "      <key>scope</key>",
  "      <string>meta.annotation variable.function, meta.annotation variable.annotation.function, meta.annotation punctuation.definition.annotation, meta.decorator, punctuation.decorator</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#000010</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>variable.parameter, meta.function.parameters</string>",
  "      <key>scope</key>",
  "      <string>variable.parameter, meta.function.parameters</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#000011</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>Built-ins</string>",
  "      <key>scope</key>",
  "      <string>constant.language, support.function.builtin</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#000012</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>entity.other.attribute-name.documentation</string>",
  "      <key>scope</key>",
  "      <string>entity.other.attribute-name.documentation</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#000013</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>Preprocessor directives</string>",
  "      <key>scope</key>",
  "      <string>keyword.control.directive, punctuation.definition.directive</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#000014</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>Type parameters</string>",
  "      <key>scope</key>",
  "      <string>punctuation.definition.typeparameters</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#000015</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>Namespaces</string>",
  "      <key>scope</key>",
  "      <string>entity.name.namespace</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#000016</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>Property names (left hand assignments in json/yaml/css)</string>",
  "      <key>scope</key>",
  "      <string>support.type.property-name.css</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#000017</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>This/Self keyword</string>",
  "      <key>scope</key>",
  "      <string>variable.language.this, variable.language.this punctuation.definition.variable</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#000018</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>Object properties</string>",
  "      <key>scope</key>",
  "      <string>variable.object.property</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#000019</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>String template interpolation</string>",
  "      <key>scope</key>",
  "      <string>string.template variable, string variable</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#00001a</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>`new` as bold</string>",
  "      <key>scope</key>",
  "      <string>keyword.operator.new</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#00001b</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>C++ extern keyword</string>",
  "      <key>scope</key>",
  "      <string>storage.modifier.specifier.extern.cpp</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#00001c</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>C++ scope resolution</string>",
  "      <key>scope</key>",
  "      <string>entity.name.scope-resolution.template.call.cpp, entity.name.scope-resolution.parameter.cpp, entity.name.scope-resolution.cpp, entity.name.scope-resolution.function.definition.cpp</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#00001d</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>C++ doc keywords</string>",
  "      <key>scope</key>",
  "      <string>storage.type.class.doxygen</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#00001e</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>C++ operators</string>",
  "      <key>scope</key>",
  "      <string>storage.modifier.reference.cpp</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#00001f</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>C# Interpolated Strings</string>",
  "      <key>scope</key>",
  "      <string>meta.interpolation.cs</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#000020</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>C# xml-style docs</string>",
  "      <key>scope</key>",
  "      <string>comment.block.documentation.cs</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#000021</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>Classes, reflecting the className color in JSX</string>",
  "      <key>scope</key>",
  "      <string>source.css entity.other.attribute-name.class.css, entity.other.attribute-name.parent-selector.css punctuation.definition.entity.css</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#000022</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>Operators</string>",
  "      <key>scope</key>",
  "      <string>punctuation.separator.operator.css</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#000023</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>Pseudo classes</string>",
  "      <key>scope</key>",
  "      <string>source.css entity.other.attribute-name.pseudo-class</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#000024</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>source.css constant.other.unicode-range</string>",
  "      <key>scope</key>",
  "      <string>source.css constant.other.unicode-range</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#000025</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>source.css variable.parameter.url</string>",
  "      <key>scope</key>",
  "      <string>source.css variable.parameter.url</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#000026</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>CSS vendored property names</string>",
  "      <key>scope</key>",
  "      <string>support.type.vendored.property-name</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#000027</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>Less/SCSS right-hand variables (@/$-prefixed)</string>",
  "      <key>scope</key>",
  "      <string>source.css meta.property-value variable, source.css meta.property-value variable.other.less, source.css meta.property-value variable.other.less punctuation.definition.variable.less, meta.definition.variable.scss</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#000028</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>CSS variables (--prefixed)</string>",
  "      <key>scope</key>",
  "      <string>source.css meta.property-list variable, meta.property-list variable.other.less, meta.property-list variable.other.less punctuation.definition.variable.less</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#000029</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>CSS Percentage values, styled the same as numbers</string>",
  "      <key>scope</key>",
  "      <string>keyword.other.unit.percentage.css</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#00002a</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>CSS Attribute selectors, styled the same as strings</string>",
  "      <key>scope</key>",
  "      <string>source.css meta.attribute-selector</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#00002b</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>JSON/YAML keys, other left-hand assignments</string>",
  "      <key>scope</key>",
  "      <string>keyword.other.definition.ini, punctuation.support.type.property-name.json, support.type.property-name.json, punctuation.support.type.property-name.toml, support.type.property-name.toml, entity.name.tag.yaml, punctuation.support.type.property-name.yaml, support.type.property-name.yaml</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#00002c</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>JSON/YAML constants</string>",
  "      <key>scope</key>",
  "      <string>constant.language.json, constant.language.yaml</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#00002d</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>YAML anchors</string>",
  "      <key>scope</key>",
  "      <string>entity.name.type.anchor.yaml, variable.other.alias.yaml</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#00002e</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>TOML tables / ini groups</string>",
  "      <key>scope</key>",
  "      <string>support.type.property-name.table, entity.name.section.group-title.ini</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#00002f</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>TOML dates</string>",
  "      <key>scope</key>",
  "      <string>constant.other.time.datetime.offset.toml</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#000030</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>YAML anchor puctuation</string>",
  "      <key>scope</key>",
  "      <string>punctuation.definition.anchor.yaml, punctuation.definition.alias.yaml</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#000031</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>YAML triple dashes</string>",
  "      <key>scope</key>",
  "      <string>entity.other.document.begin.yaml</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#000032</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>Markup Diff</string>",
  "      <key>scope</key>",
  "      <string>markup.changed.diff</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#000033</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>Diff</string>",
  "      <key>scope</key>",
  "      <string>meta.diff.header.from-file, meta.diff.header.to-file, punctuation.definition.from-file.diff, punctuation.definition.to-file.diff</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#000034</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>Diff Inserted</string>",
  "      <key>scope</key>",
  "      <string>markup.inserted.diff</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#000035</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>Diff Deleted</string>",
  "      <key>scope</key>",
  "      <string>markup.deleted.diff</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#000036</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>dotenv left-hand side assignments</string>",
  "      <key>scope</key>",
  "      <string>variable.other.env</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#000037</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>dotenv reference to existing env variable</string>",
  "      <key>scope</key>",
  "      <string>string.quoted variable.other.env</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#000038</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>GDScript functions</string>",
  "      <key>scope</key>",
  "      <string>support.function.builtin.gdscript</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#000039</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>GDScript constants</string>",
  "      <key>scope</key>",
  "      <string>constant.language.gdscript</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#00003a</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>Comment keywords</string>",
  "      <key>scope</key>",
  "      <string>comment meta.annotation.go</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#00003b</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>go:embed, go:build, etc.</string>",
  "      <key>scope</key>",
  "      <string>comment meta.annotation.parameters.go</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#00003c</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>Go constants (nil, true, false)</string>",
  "      <key>scope</key>",
  "      <string>constant.language.go</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#00003d</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>GraphQL variables</string>",
  "      <key>scope</key>",
  "      <string>variable.graphql</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#00003e</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>GraphQL aliases</string>",
  "      <key>scope</key>",
  "      <string>string.unquoted.alias.graphql</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#00003f</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>GraphQL enum members</string>",
  "      <key>scope</key>",
  "      <string>constant.character.enum.graphql</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#000040</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>GraphQL field in types</string>",
  "      <key>scope</key>",
  "      <string>meta.objectvalues.graphql constant.object.key.graphql string.unquoted.graphql</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#000041</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>HTML/XML DOCTYPE as keyword</string>",
  "      <key>scope</key>",
  "      <string>keyword.other.doctype, meta.tag.sgml.doctype punctuation.definition.tag, meta.tag.metadata.doctype entity.name.tag, meta.tag.metadata.doctype punctuation.definition.tag</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#000042</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>HTML/XML-like &lt;tags/&gt;</string>",
  "      <key>scope</key>",
  "      <string>entity.name.tag</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#000043</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>Special characters like &amp;amp;</string>",
  "      <key>scope</key>",
  "      <string>text.html constant.character.entity, text.html constant.character.entity punctuation, constant.character.entity.xml, constant.character.entity.xml punctuation, constant.character.entity.js.jsx, constant.charactger.entity.js.jsx punctuation, constant.character.entity.tsx, constant.character.entity.tsx punctuation</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#000044</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>HTML/XML tag attribute values</string>",
  "      <key>scope</key>",
  "      <string>entity.other.attribute-name</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#000045</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>Components</string>",
  "      <key>scope</key>",
  "      <string>support.class.component, support.class.component.jsx, support.class.component.tsx, support.class.component.vue</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#000046</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>Annotations</string>",
  "      <key>scope</key>",
  "      <string>punctuation.definition.annotation, storage.type.annotation</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#000047</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>Java enums</string>",
  "      <key>scope</key>",
  "      <string>constant.other.enum.java</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#000048</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>Java imports</string>",
  "      <key>scope</key>",
  "      <string>storage.modifier.import.java</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#000049</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>Javadoc</string>",
  "      <key>scope</key>",
  "      <string>comment.block.javadoc.java keyword.other.documentation.javadoc.java</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#00004a</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>Exported Variable</string>",
  "      <key>scope</key>",
  "      <string>meta.export variable.other.readwrite.js</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#00004b</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>JS/TS constants &amp; properties</string>",
  "      <key>scope</key>",
  "      <string>variable.other.constant.js, variable.other.constant.ts, variable.other.property.js, variable.other.property.ts</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#00004c</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>JSDoc; these are mainly params, so styled as such</string>",
  "      <key>scope</key>",
  "      <string>variable.other.jsdoc, comment.block.documentation variable.other</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#00004d</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>JSDoc keywords</string>",
  "      <key>scope</key>",
  "      <string>storage.type.class.jsdoc</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#00004e</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>support.type.object.console.js</string>",
  "      <key>scope</key>",
  "      <string>support.type.object.console.js</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#00004f</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>Node constants as keywords (module, etc.)</string>",
  "      <key>scope</key>",
  "      <string>support.constant.node, support.type.object.module.js</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#000050</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>implements as keyword</string>",
  "      <key>scope</key>",
  "      <string>storage.modifier.implements</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#000051</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>Builtin types</string>",
  "      <key>scope</key>",
  "      <string>constant.language.null.js, constant.language.null.ts, constant.language.undefined.js, constant.language.undefined.ts, support.type.builtin.ts</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#000052</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>variable.parameter.generic</string>",
  "      <key>scope</key>",
  "      <string>variable.parameter.generic</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#000053</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>Arrow functions</string>",
  "      <key>scope</key>",
  "      <string>keyword.declaration.function.arrow.js, storage.type.function.arrow.ts</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#000054</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>Decorator punctuations (decorators inherit from blue functions, instead of styleguide peach)</string>",
  "      <key>scope</key>",
  "      <string>punctuation.decorator.ts</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#000055</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>Extra JS/TS keywords</string>",
  "      <key>scope</key>",
  "      <string>keyword.operator.expression.in.js, keyword.operator.expression.in.ts, keyword.operator.expression.infer.ts, keyword.operator.expression.instanceof.js, keyword.operator.expression.instanceof.ts, keyword.operator.expression.is, keyword.operator.expression.keyof.ts, keyword.operator.expression.of.js, keyword.operator.expression.of.ts, keyword.operator.expression.typeof.ts</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#000056</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>Julia macros</string>",
  "      <key>scope</key>",
  "      <string>support.function.macro.julia</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#000057</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>Julia language constants (true, false)</string>",
  "      <key>scope</key>",
  "      <string>constant.language.julia</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#000058</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>Julia other constants (these seem to be arguments inside arrays)</string>",
  "      <key>scope</key>",
  "      <string>constant.other.symbol.julia</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#000059</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>LaTeX preamble</string>",
  "      <key>scope</key>",
  "      <string>text.tex keyword.control.preamble</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#00005a</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>LaTeX be functions</string>",
  "      <key>scope</key>",
  "      <string>text.tex support.function.be</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#00005b</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>LaTeX math</string>",
  "      <key>scope</key>",
  "      <string>constant.other.general.math.tex</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#00005c</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>Liquid Builtin Objects &amp; User Defined Variables</string>",
  "      <key>scope</key>",
  "      <string>variable.language.liquid</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#00005d</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>Lua docstring keywords</string>",
  "      <key>scope</key>",
  "      <string>comment.line.double-dash.documentation.lua storage.type.annotation.lua</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#00005e</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>Lua docstring variables</string>",
  "      <key>scope</key>",
  "      <string>comment.line.double-dash.documentation.lua entity.name.variable.lua, comment.line.double-dash.documentation.lua variable.lua</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#00005f</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>heading.1.markdown punctuation.definition.heading.markdown, heading.1.markdown, heading.1.quarto punctuation.definition.heading.quarto, heading.1.quarto, markup.heading.atx.1.mdx, markup.heading.atx.1.mdx punctuation.definition.heading.mdx, markup.heading.setext.1.markdown, markup.heading.heading-0.asciidoc</string>",
  "      <key>scope</key>",
  "      <string>heading.1.markdown punctuation.definition.heading.markdown, heading.1.markdown, heading.1.quarto punctuation.definition.heading.quarto, heading.1.quarto, markup.heading.atx.1.mdx, markup.heading.atx.1.mdx punctuation.definition.heading.mdx, markup.heading.setext.1.markdown, markup.heading.heading-0.asciidoc</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#000060</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>heading.2.markdown punctuation.definition.heading.markdown, heading.2.markdown, heading.2.quarto punctuation.definition.heading.quarto, heading.2.quarto, markup.heading.atx.2.mdx, markup.heading.atx.2.mdx punctuation.definition.heading.mdx, markup.heading.setext.2.markdown, markup.heading.heading-1.asciidoc</string>",
  "      <key>scope</key>",
  "      <string>heading.2.markdown punctuation.definition.heading.markdown, heading.2.markdown, heading.2.quarto punctuation.definition.heading.quarto, heading.2.quarto, markup.heading.atx.2.mdx, markup.heading.atx.2.mdx punctuation.definition.heading.mdx, markup.heading.setext.2.markdown, markup.heading.heading-1.asciidoc</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#000061</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>heading.3.markdown punctuation.definition.heading.markdown, heading.3.markdown, heading.3.quarto punctuation.definition.heading.quarto, heading.3.quarto, markup.heading.atx.3.mdx, markup.heading.atx.3.mdx punctuation.definition.heading.mdx, markup.heading.heading-2.asciidoc</string>",
  "      <key>scope</key>",
  "      <string>heading.3.markdown punctuation.definition.heading.markdown, heading.3.markdown, heading.3.quarto punctuation.definition.heading.quarto, heading.3.quarto, markup.heading.atx.3.mdx, markup.heading.atx.3.mdx punctuation.definition.heading.mdx, markup.heading.heading-2.asciidoc</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#000062</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>heading.4.markdown punctuation.definition.heading.markdown, heading.4.markdown, heading.4.quarto punctuation.definition.heading.quarto, heading.4.quarto, markup.heading.atx.4.mdx, markup.heading.atx.4.mdx punctuation.definition.heading.mdx, markup.heading.heading-3.asciidoc</string>",
  "      <key>scope</key>",
  "      <string>heading.4.markdown punctuation.definition.heading.markdown, heading.4.markdown, heading.4.quarto punctuation.definition.heading.quarto, heading.4.quarto, markup.heading.atx.4.mdx, markup.heading.atx.4.mdx punctuation.definition.heading.mdx, markup.heading.heading-3.asciidoc</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#000063</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>heading.5.markdown punctuation.definition.heading.markdown, heading.5.markdown, heading.5.quarto punctuation.definition.heading.quarto, heading.5.quarto, markup.heading.atx.5.mdx, markup.heading.atx.5.mdx punctuation.definition.heading.mdx, markup.heading.heading-4.asciidoc</string>",
  "      <key>scope</key>",
  "      <string>heading.5.markdown punctuation.definition.heading.markdown, heading.5.markdown, heading.5.quarto punctuation.definition.heading.quarto, heading.5.quarto, markup.heading.atx.5.mdx, markup.heading.atx.5.mdx punctuation.definition.heading.mdx, markup.heading.heading-4.asciidoc</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#000064</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>heading.6.markdown punctuation.definition.heading.markdown, heading.6.markdown, heading.6.quarto punctuation.definition.heading.quarto, heading.6.quarto, markup.heading.atx.6.mdx, markup.heading.atx.6.mdx punctuation.definition.heading.mdx, markup.heading.heading-5.asciidoc</string>",
  "      <key>scope</key>",
  "      <string>heading.6.markdown punctuation.definition.heading.markdown, heading.6.markdown, heading.6.quarto punctuation.definition.heading.quarto, heading.6.quarto, markup.heading.atx.6.mdx, markup.heading.atx.6.mdx punctuation.definition.heading.mdx, markup.heading.heading-5.asciidoc</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#000065</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>markup.bold</string>",
  "      <key>scope</key>",
  "      <string>markup.bold</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#000066</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>markup.italic</string>",
  "      <key>scope</key>",
  "      <string>markup.italic</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#000067</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>markup.strikethrough</string>",
  "      <key>scope</key>",
  "      <string>markup.strikethrough</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#000068</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>Markdown auto links</string>",
  "      <key>scope</key>",
  "      <string>punctuation.definition.link, markup.underline.link</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#000069</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>Markdown links</string>",
  "      <key>scope</key>",
  "      <string>text.html.markdown punctuation.definition.link.title, text.html.quarto punctuation.definition.link.title, string.other.link.title.markdown, string.other.link.title.quarto, markup.link, punctuation.definition.constant.markdown, punctuation.definition.constant.quarto, constant.other.reference.link.markdown, constant.other.reference.link.quarto, markup.substitution.attribute-reference</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#00006a</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>Markdown code spans</string>",
  "      <key>scope</key>",
  "      <string>punctuation.definition.raw.markdown, punctuation.definition.raw.quarto, markup.inline.raw.string.markdown, markup.inline.raw.string.quarto, markup.raw.block.markdown, markup.raw.block.quarto</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#00006b</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>Markdown triple backtick language identifier</string>",
  "      <key>scope</key>",
  "      <string>fenced_code.block.language</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#00006c</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>Markdown triple backticks</string>",
  "      <key>scope</key>",
  "      <string>markup.fenced_code.block punctuation.definition, markup.raw support.asciidoc</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#00006d</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>Markdown quotes</string>",
  "      <key>scope</key>",
  "      <string>markup.quote, punctuation.definition.quote.begin</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#00006e</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>Markdown separators</string>",
  "      <key>scope</key>",
  "      <string>meta.separator.markdown</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#00006f</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>Markdown list bullets</string>",
  "      <key>scope</key>",
  "      <string>punctuation.definition.list.begin.markdown, punctuation.definition.list.begin.quarto, markup.list.bullet</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#000070</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>Quarto headings</string>",
  "      <key>scope</key>",
  "      <string>markup.heading.quarto</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#000071</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>Nix attribute names</string>",
  "      <key>scope</key>",
  "      <string>entity.other.attribute-name.multipart.nix, entity.other.attribute-name.single.nix</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#000072</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>Nix parameter names</string>",
  "      <key>scope</key>",
  "      <string>variable.parameter.name.nix</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#000073</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>Nix interpolated parameter names</string>",
  "      <key>scope</key>",
  "      <string>meta.embedded variable.parameter.name.nix</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#000074</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>Nix paths</string>",
  "      <key>scope</key>",
  "      <string>string.unquoted.path.nix</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#000075</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>PHP Attributes</string>",
  "      <key>scope</key>",
  "      <string>support.attribute.builtin, meta.attribute.php</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#000076</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>PHP Parameters (needed for the leading dollar sign)</string>",
  "      <key>scope</key>",
  "      <string>meta.function.parameters.php punctuation.definition.variable.php</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#000077</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>PHP Constants (null, __FILE__, etc.)</string>",
  "      <key>scope</key>",
  "      <string>constant.language.php</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#000078</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>PHP functions</string>",
  "      <key>scope</key>",
  "      <string>text.html.php support.function</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#000079</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>PHPdoc keywords</string>",
  "      <key>scope</key>",
  "      <string>keyword.other.phpdoc.php</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#00007a</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>Python argument functions reset to text, otherwise they inherit blue from function-call</string>",
  "      <key>scope</key>",
  "      <string>support.variable.magic.python, meta.function-call.arguments.python</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#00007b</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>Python double underscore functions</string>",
  "      <key>scope</key>",
  "      <string>support.function.magic.python</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#00007c</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>Python `self` keyword</string>",
  "      <key>scope</key>",
  "      <string>variable.parameter.function.language.special.self.python, variable.language.special.self.python</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#00007d</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>python keyword flow/logical (for ... in)</string>",
  "      <key>scope</key>",
  "      <string>keyword.control.flow.python, keyword.operator.logical.python</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#00007e</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>python storage type</string>",
  "      <key>scope</key>",
  "      <string>storage.type.function.python</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#00007f</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>python function support</string>",
  "      <key>scope</key>",
  "      <string>support.token.decorator.python, meta.function.decorator.identifier.python</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#000080</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>python function calls</string>",
  "      <key>scope</key>",
  "      <string>meta.function-call.python</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#000081</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>python function decorators</string>",
  "      <key>scope</key>",
  "      <string>entity.name.function.decorator.python, punctuation.definition.decorator.python</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#000082</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>python placeholder reset to normal string</string>",
  "      <key>scope</key>",
  "      <string>constant.character.format.placeholder.other.python</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#000083</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>Python exception &amp; builtins such as exit()</string>",
  "      <key>scope</key>",
  "      <string>support.type.exception.python, support.function.builtin.python</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#000084</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>entity.name.type</string>",
  "      <key>scope</key>",
  "      <string>support.type.python</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#000085</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>python constants (True/False)</string>",
  "      <key>scope</key>",
  "      <string>constant.language.python</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#000086</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>Arguments accessed later in the function body</string>",
  "      <key>scope</key>",
  "      <string>meta.indexed-name.python, meta.item-access.python</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#000087</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>Python f-strings/binary/unicode storage types</string>",
  "      <key>scope</key>",
  "      <string>storage.type.string.python</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#000088</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>Python type hints</string>",
  "      <key>scope</key>",
  "      <string>meta.function.parameters.python</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#000089</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>Regex string begin/end in JS/TS</string>",
  "      <key>scope</key>",
  "      <string>string.regexp punctuation.definition.string.begin, string.regexp punctuation.definition.string.end</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#00008a</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>Regex anchors (^, $)</string>",
  "      <key>scope</key>",
  "      <string>keyword.control.anchor.regexp</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#00008b</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>Regex regular string match</string>",
  "      <key>scope</key>",
  "      <string>string.regexp.ts</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#00008c</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>Regex group parenthesis &amp; backreference (\\1, \\2, \\3, ...)</string>",
  "      <key>scope</key>",
  "      <string>punctuation.definition.group.regexp, keyword.other.back-reference.regexp</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#00008d</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>Regex character class []</string>",
  "      <key>scope</key>",
  "      <string>punctuation.definition.character-class.regexp</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#00008e</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>Regex character classes (\\d, \\w, \\s)</string>",
  "      <key>scope</key>",
  "      <string>constant.other.character-class.regexp</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#00008f</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>Regex range</string>",
  "      <key>scope</key>",
  "      <string>constant.other.character-class.range.regexp</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#000090</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>Regex quantifier</string>",
  "      <key>scope</key>",
  "      <string>keyword.operator.quantifier.regexp</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#000091</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>Regex constant/numeric</string>",
  "      <key>scope</key>",
  "      <string>constant.character.numeric.regexp</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#000092</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>Regex lookaheads, negative lookaheads, lookbehinds, negative lookbehinds</string>",
  "      <key>scope</key>",
  "      <string>punctuation.definition.group.no-capture.regexp, meta.assertion.look-ahead.regexp, meta.assertion.negative-look-ahead.regexp</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#000093</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>Rust attribute</string>",
  "      <key>scope</key>",
  "      <string>meta.annotation.rust, meta.annotation.rust punctuation, meta.attribute.rust, punctuation.definition.attribute.rust</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#000094</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>Rust attribute strings</string>",
  "      <key>scope</key>",
  "      <string>meta.attribute.rust string.quoted.double.rust, meta.attribute.rust string.quoted.single.char.rust</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#000095</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>Rust keyword</string>",
  "      <key>scope</key>",
  "      <string>entity.name.function.macro.rules.rust, storage.type.module.rust, storage.modifier.rust, storage.type.struct.rust, storage.type.enum.rust, storage.type.trait.rust, storage.type.union.rust, storage.type.impl.rust, storage.type.rust, storage.type.function.rust, storage.type.type.rust</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#000096</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>Rust u/i32, u/i64, etc.</string>",
  "      <key>scope</key>",
  "      <string>entity.name.type.numeric.rust</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#000097</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>Rust generic</string>",
  "      <key>scope</key>",
  "      <string>meta.generic.rust</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#000098</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>Rust impl</string>",
  "      <key>scope</key>",
  "      <string>entity.name.impl.rust</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#000099</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>Rust module</string>",
  "      <key>scope</key>",
  "      <string>entity.name.module.rust</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#00009a</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>Rust trait</string>",
  "      <key>scope</key>",
  "      <string>entity.name.trait.rust</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#00009b</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>Rust struct</string>",
  "      <key>scope</key>",
  "      <string>storage.type.source.rust</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#00009c</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>Rust union</string>",
  "      <key>scope</key>",
  "      <string>entity.name.union.rust</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#00009d</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>Rust enum member</string>",
  "      <key>scope</key>",
  "      <string>meta.enum.rust storage.type.source.rust</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#00009e</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>Rust macro</string>",
  "      <key>scope</key>",
  "      <string>support.macro.rust, meta.macro.rust support.function.rust, entity.name.function.macro.rust</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#00009f</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>Rust lifetime</string>",
  "      <key>scope</key>",
  "      <string>storage.modifier.lifetime.rust, entity.name.type.lifetime</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#0000a0</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>Rust string formatting</string>",
  "      <key>scope</key>",
  "      <string>string.quoted.double.rust constant.other.placeholder.rust</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#0000a1</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>Rust return type generic</string>",
  "      <key>scope</key>",
  "      <string>meta.function.return-type.rust meta.generic.rust storage.type.rust</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#0000a2</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>Rust functions</string>",
  "      <key>scope</key>",
  "      <string>meta.function.call.rust</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#0000a3</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>Rust angle brackets</string>",
  "      <key>scope</key>",
  "      <string>punctuation.brackets.angle.rust</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#0000a4</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>Rust constants</string>",
  "      <key>scope</key>",
  "      <string>constant.other.caps.rust</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#0000a5</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>Rust function parameters</string>",
  "      <key>scope</key>",
  "      <string>meta.function.definition.rust variable.other.rust</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#0000a6</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>Rust closure variables</string>",
  "      <key>scope</key>",
  "      <string>meta.function.call.rust variable.other.rust</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#0000a7</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>Rust self</string>",
  "      <key>scope</key>",
  "      <string>variable.language.self.rust</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#0000a8</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>Rust metavariable names</string>",
  "      <key>scope</key>",
  "      <string>variable.other.metavariable.name.rust, meta.macro.metavariable.rust keyword.operator.macro.dollar.rust</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#0000a9</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>Shell shebang</string>",
  "      <key>scope</key>",
  "      <string>comment.line.shebang, comment.line.shebang punctuation.definition.comment, comment.line.shebang, punctuation.definition.comment.shebang.shell, meta.shebang.shell</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#0000aa</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>Shell shebang command</string>",
  "      <key>scope</key>",
  "      <string>comment.line.shebang constant.language</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#0000ab</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>Shell interpolated command</string>",
  "      <key>scope</key>",
  "      <string>meta.function-call.arguments.shell punctuation.definition.variable.shell, meta.function-call.arguments.shell punctuation.section.interpolation, meta.function-call.arguments.shell punctuation.definition.variable.shell, meta.function-call.arguments.shell punctuation.section.interpolation</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#0000ac</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>Shell interpolated command variable</string>",
  "      <key>scope</key>",
  "      <string>meta.string meta.interpolation.parameter.shell variable.other.readwrite</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#0000ad</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>source.shell punctuation.section.interpolation, punctuation.definition.evaluation.backticks.shell</string>",
  "      <key>scope</key>",
  "      <string>source.shell punctuation.section.interpolation, punctuation.definition.evaluation.backticks.shell</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#0000ae</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>Shell EOF</string>",
  "      <key>scope</key>",
  "      <string>entity.name.tag.heredoc.shell</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#0000af</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>Shell quoted variable</string>",
  "      <key>scope</key>",
  "      <string>string.quoted.double.shell variable.other.normal.shell</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#0000b0</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>markup.heading.typst</string>",
  "      <key>scope</key>",
  "      <string>markup.heading.typst</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#0000b1</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>JSON Keys</string>",
  "      <key>scope</key>",
  "      <string>source.json meta.mapping.key string</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#0000b2</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>JSON key surrounding quotes</string>",
  "      <key>scope</key>",
  "      <string>source.json meta.mapping.key punctuation.definition.string.begin, source.json meta.mapping.key punctuation.definition.string.end</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#0000b3</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>markup.heading.synopsis.man, markup.heading.title.man, markup.heading.other.man, markup.heading.env.man</string>",
  "      <key>scope</key>",
  "      <string>markup.heading.synopsis.man, markup.heading.title.man, markup.heading.other.man, markup.heading.env.man</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#0000b4</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>markup.heading.commands.man</string>",
  "      <key>scope</key>",
  "      <string>markup.heading.commands.man</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#0000b5</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>markup.heading.env.man</string>",
  "      <key>scope</key>",
  "      <string>markup.heading.env.man</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#0000b6</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>Man page options</string>",
  "      <key>scope</key>",
  "      <string>entity.name</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#0000b7</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>markup.heading.1.markdown</string>",
  "      <key>scope</key>",
  "      <string>markup.heading.1.markdown</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#0000b8</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>markup.heading.2.markdown</string>",
  "      <key>scope</key>",
  "      <string>markup.heading.2.markdown</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#0000b9</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>markup.heading.markdown</string>",
  "      <key>scope</key>",
  "      <string>markup.heading.markdown</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#0000ba</string>",
  "      </dict>",
  "    </dict>",
  "  </array>",
  "</dict>",
  "</plist>",
).join("\n"))

#let _output-syntax-theme = _input-syntax-theme

#let _paged-syntax-theme = bytes((
  "<?xml version=\"1.0\" encoding=\"UTF-8\"?>",
  "<!DOCTYPE plist PUBLIC \"-//Apple//DTD PLIST 1.0//EN\" \"http://www.apple.com/DTDs/PropertyList-1.0.dtd\">",
  "<plist version=\"1.0\">",
  "<dict>",
  "  <key>name</key>",
  "  <string>Calepin Paged Syntax</string>",
  "  <key>settings</key>",
  "  <array>",
  "    <dict>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#4c4f69</string>",
  "        <key>background</key>",
  "        <string>#eff1f5</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>Basic text &amp; variable names (incl. leading punctuation)</string>",
  "      <key>scope</key>",
  "      <string>text, source, variable.other.readwrite, punctuation.definition.variable</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#4c4f69</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>Parentheses, Brackets, Braces</string>",
  "      <key>scope</key>",
  "      <string>punctuation</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#7c7f93</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>Comments</string>",
  "      <key>scope</key>",
  "      <string>comment, punctuation.definition.comment</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#7c7f93</string>",
  "        <key>fontStyle</key>",
  "        <string>italic</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>string, punctuation.definition.string</string>",
  "      <key>scope</key>",
  "      <string>string, punctuation.definition.string</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#40a02b</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>constant.character.escape</string>",
  "      <key>scope</key>",
  "      <string>constant.character.escape</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#ea76cb</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>Booleans, constants, numbers</string>",
  "      <key>scope</key>",
  "      <string>constant.numeric, variable.other.constant, entity.name.constant, constant.language.boolean, constant.language.false, constant.language.true, keyword.other.unit.user-defined, keyword.other.unit.suffix.floating-point</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#fe640b</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>keyword, keyword.operator.word, keyword.operator.new, variable.language.super, support.type.primitive, storage.type, storage.modifier, punctuation.definition.keyword</string>",
  "      <key>scope</key>",
  "      <string>keyword, keyword.operator.word, keyword.operator.new, variable.language.super, support.type.primitive, storage.type, storage.modifier, punctuation.definition.keyword</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#8839ef</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>entity.name.tag.documentation</string>",
  "      <key>scope</key>",
  "      <string>entity.name.tag.documentation</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#8839ef</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>Punctuation</string>",
  "      <key>scope</key>",
  "      <string>keyword.operator, punctuation.accessor, punctuation.definition.generic, meta.function.closure punctuation.section.parameters, punctuation.definition.tag, punctuation.separator.key-value</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#179299</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>entity.name.function, meta.function-call.method, support.function, support.function.misc, variable.function</string>",
  "      <key>scope</key>",
  "      <string>entity.name.function, meta.function-call.method, support.function, support.function.misc, variable.function</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#1e66f5</string>",
  "        <key>fontStyle</key>",
  "        <string>italic</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>Classes</string>",
  "      <key>scope</key>",
  "      <string>entity.name.class, entity.other.inherited-class, support.class, meta.function-call.constructor, entity.name.struct</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#df8e1d</string>",
  "        <key>fontStyle</key>",
  "        <string>italic</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>Enum</string>",
  "      <key>scope</key>",
  "      <string>entity.name.enum</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#df8e1d</string>",
  "        <key>fontStyle</key>",
  "        <string>italic</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>Enum member</string>",
  "      <key>scope</key>",
  "      <string>meta.enum variable.other.readwrite, variable.other.enummember</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#179299</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>Object properties</string>",
  "      <key>scope</key>",
  "      <string>meta.property.object</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#179299</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>Types</string>",
  "      <key>scope</key>",
  "      <string>meta.type, meta.type-alias, support.type, entity.name.type</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#df8e1d</string>",
  "        <key>fontStyle</key>",
  "        <string>italic</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>Decorators</string>",
  "      <key>scope</key>",
  "      <string>meta.annotation variable.function, meta.annotation variable.annotation.function, meta.annotation punctuation.definition.annotation, meta.decorator, punctuation.decorator</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#fe640b</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>variable.parameter, meta.function.parameters</string>",
  "      <key>scope</key>",
  "      <string>variable.parameter, meta.function.parameters</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#e64553</string>",
  "        <key>fontStyle</key>",
  "        <string>italic</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>Built-ins</string>",
  "      <key>scope</key>",
  "      <string>constant.language, support.function.builtin</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#d20f39</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>entity.other.attribute-name.documentation</string>",
  "      <key>scope</key>",
  "      <string>entity.other.attribute-name.documentation</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#d20f39</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>Preprocessor directives</string>",
  "      <key>scope</key>",
  "      <string>keyword.control.directive, punctuation.definition.directive</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#df8e1d</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>Type parameters</string>",
  "      <key>scope</key>",
  "      <string>punctuation.definition.typeparameters</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#04a5e5</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>Namespaces</string>",
  "      <key>scope</key>",
  "      <string>entity.name.namespace</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#df8e1d</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>Property names (left hand assignments in json/yaml/css)</string>",
  "      <key>scope</key>",
  "      <string>support.type.property-name.css</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#1e66f5</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>This/Self keyword</string>",
  "      <key>scope</key>",
  "      <string>variable.language.this, variable.language.this punctuation.definition.variable</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#d20f39</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>Object properties</string>",
  "      <key>scope</key>",
  "      <string>variable.object.property</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#4c4f69</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>String template interpolation</string>",
  "      <key>scope</key>",
  "      <string>string.template variable, string variable</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#4c4f69</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>`new` as bold</string>",
  "      <key>scope</key>",
  "      <string>keyword.operator.new</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>fontStyle</key>",
  "        <string>bold</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>C++ extern keyword</string>",
  "      <key>scope</key>",
  "      <string>storage.modifier.specifier.extern.cpp</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#8839ef</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>C++ scope resolution</string>",
  "      <key>scope</key>",
  "      <string>entity.name.scope-resolution.template.call.cpp, entity.name.scope-resolution.parameter.cpp, entity.name.scope-resolution.cpp, entity.name.scope-resolution.function.definition.cpp</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#df8e1d</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>C++ doc keywords</string>",
  "      <key>scope</key>",
  "      <string>storage.type.class.doxygen</string>",
  "      <key>settings</key>",
  "      <dict>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>C++ operators</string>",
  "      <key>scope</key>",
  "      <string>storage.modifier.reference.cpp</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#179299</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>C# Interpolated Strings</string>",
  "      <key>scope</key>",
  "      <string>meta.interpolation.cs</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#4c4f69</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>C# xml-style docs</string>",
  "      <key>scope</key>",
  "      <string>comment.block.documentation.cs</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#4c4f69</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>Classes, reflecting the className color in JSX</string>",
  "      <key>scope</key>",
  "      <string>source.css entity.other.attribute-name.class.css, entity.other.attribute-name.parent-selector.css punctuation.definition.entity.css</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#df8e1d</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>Operators</string>",
  "      <key>scope</key>",
  "      <string>punctuation.separator.operator.css</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#179299</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>Pseudo classes</string>",
  "      <key>scope</key>",
  "      <string>source.css entity.other.attribute-name.pseudo-class</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#179299</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>source.css constant.other.unicode-range</string>",
  "      <key>scope</key>",
  "      <string>source.css constant.other.unicode-range</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#fe640b</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>source.css variable.parameter.url</string>",
  "      <key>scope</key>",
  "      <string>source.css variable.parameter.url</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#40a02b</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>CSS vendored property names</string>",
  "      <key>scope</key>",
  "      <string>support.type.vendored.property-name</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#04a5e5</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>Less/SCSS right-hand variables (@/$-prefixed)</string>",
  "      <key>scope</key>",
  "      <string>source.css meta.property-value variable, source.css meta.property-value variable.other.less, source.css meta.property-value variable.other.less punctuation.definition.variable.less, meta.definition.variable.scss</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#e64553</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>CSS variables (--prefixed)</string>",
  "      <key>scope</key>",
  "      <string>source.css meta.property-list variable, meta.property-list variable.other.less, meta.property-list variable.other.less punctuation.definition.variable.less</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#1e66f5</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>CSS Percentage values, styled the same as numbers</string>",
  "      <key>scope</key>",
  "      <string>keyword.other.unit.percentage.css</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#fe640b</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>CSS Attribute selectors, styled the same as strings</string>",
  "      <key>scope</key>",
  "      <string>source.css meta.attribute-selector</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#40a02b</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>JSON/YAML keys, other left-hand assignments</string>",
  "      <key>scope</key>",
  "      <string>keyword.other.definition.ini, punctuation.support.type.property-name.json, support.type.property-name.json, punctuation.support.type.property-name.toml, support.type.property-name.toml, entity.name.tag.yaml, punctuation.support.type.property-name.yaml, support.type.property-name.yaml</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#1e66f5</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>JSON/YAML constants</string>",
  "      <key>scope</key>",
  "      <string>constant.language.json, constant.language.yaml</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#fe640b</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>YAML anchors</string>",
  "      <key>scope</key>",
  "      <string>entity.name.type.anchor.yaml, variable.other.alias.yaml</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#df8e1d</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>TOML tables / ini groups</string>",
  "      <key>scope</key>",
  "      <string>support.type.property-name.table, entity.name.section.group-title.ini</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#df8e1d</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>TOML dates</string>",
  "      <key>scope</key>",
  "      <string>constant.other.time.datetime.offset.toml</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#ea76cb</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>YAML anchor puctuation</string>",
  "      <key>scope</key>",
  "      <string>punctuation.definition.anchor.yaml, punctuation.definition.alias.yaml</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#ea76cb</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>YAML triple dashes</string>",
  "      <key>scope</key>",
  "      <string>entity.other.document.begin.yaml</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#ea76cb</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>Markup Diff</string>",
  "      <key>scope</key>",
  "      <string>markup.changed.diff</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#fe640b</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>Diff</string>",
  "      <key>scope</key>",
  "      <string>meta.diff.header.from-file, meta.diff.header.to-file, punctuation.definition.from-file.diff, punctuation.definition.to-file.diff</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#1e66f5</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>Diff Inserted</string>",
  "      <key>scope</key>",
  "      <string>markup.inserted.diff</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#40a02b</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>Diff Deleted</string>",
  "      <key>scope</key>",
  "      <string>markup.deleted.diff</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#d20f39</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>dotenv left-hand side assignments</string>",
  "      <key>scope</key>",
  "      <string>variable.other.env</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#1e66f5</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>dotenv reference to existing env variable</string>",
  "      <key>scope</key>",
  "      <string>string.quoted variable.other.env</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#4c4f69</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>GDScript functions</string>",
  "      <key>scope</key>",
  "      <string>support.function.builtin.gdscript</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#1e66f5</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>GDScript constants</string>",
  "      <key>scope</key>",
  "      <string>constant.language.gdscript</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#fe640b</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>Comment keywords</string>",
  "      <key>scope</key>",
  "      <string>comment meta.annotation.go</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#e64553</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>go:embed, go:build, etc.</string>",
  "      <key>scope</key>",
  "      <string>comment meta.annotation.parameters.go</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#fe640b</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>Go constants (nil, true, false)</string>",
  "      <key>scope</key>",
  "      <string>constant.language.go</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#fe640b</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>GraphQL variables</string>",
  "      <key>scope</key>",
  "      <string>variable.graphql</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#4c4f69</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>GraphQL aliases</string>",
  "      <key>scope</key>",
  "      <string>string.unquoted.alias.graphql</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#dd7878</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>GraphQL enum members</string>",
  "      <key>scope</key>",
  "      <string>constant.character.enum.graphql</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#179299</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>GraphQL field in types</string>",
  "      <key>scope</key>",
  "      <string>meta.objectvalues.graphql constant.object.key.graphql string.unquoted.graphql</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#dd7878</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>HTML/XML DOCTYPE as keyword</string>",
  "      <key>scope</key>",
  "      <string>keyword.other.doctype, meta.tag.sgml.doctype punctuation.definition.tag, meta.tag.metadata.doctype entity.name.tag, meta.tag.metadata.doctype punctuation.definition.tag</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#8839ef</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>HTML/XML-like &lt;tags/&gt;</string>",
  "      <key>scope</key>",
  "      <string>entity.name.tag</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#1e66f5</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>Special characters like &amp;amp;</string>",
  "      <key>scope</key>",
  "      <string>text.html constant.character.entity, text.html constant.character.entity punctuation, constant.character.entity.xml, constant.character.entity.xml punctuation, constant.character.entity.js.jsx, constant.charactger.entity.js.jsx punctuation, constant.character.entity.tsx, constant.character.entity.tsx punctuation</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#d20f39</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>HTML/XML tag attribute values</string>",
  "      <key>scope</key>",
  "      <string>entity.other.attribute-name</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#df8e1d</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>Components</string>",
  "      <key>scope</key>",
  "      <string>support.class.component, support.class.component.jsx, support.class.component.tsx, support.class.component.vue</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#ea76cb</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>Annotations</string>",
  "      <key>scope</key>",
  "      <string>punctuation.definition.annotation, storage.type.annotation</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#fe640b</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>Java enums</string>",
  "      <key>scope</key>",
  "      <string>constant.other.enum.java</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#179299</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>Java imports</string>",
  "      <key>scope</key>",
  "      <string>storage.modifier.import.java</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#4c4f69</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>Javadoc</string>",
  "      <key>scope</key>",
  "      <string>comment.block.javadoc.java keyword.other.documentation.javadoc.java</string>",
  "      <key>settings</key>",
  "      <dict>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>Exported Variable</string>",
  "      <key>scope</key>",
  "      <string>meta.export variable.other.readwrite.js</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#e64553</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>JS/TS constants &amp; properties</string>",
  "      <key>scope</key>",
  "      <string>variable.other.constant.js, variable.other.constant.ts, variable.other.property.js, variable.other.property.ts</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#4c4f69</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>JSDoc; these are mainly params, so styled as such</string>",
  "      <key>scope</key>",
  "      <string>variable.other.jsdoc, comment.block.documentation variable.other</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#e64553</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>JSDoc keywords</string>",
  "      <key>scope</key>",
  "      <string>storage.type.class.jsdoc</string>",
  "      <key>settings</key>",
  "      <dict>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>support.type.object.console.js</string>",
  "      <key>scope</key>",
  "      <string>support.type.object.console.js</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#4c4f69</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>Node constants as keywords (module, etc.)</string>",
  "      <key>scope</key>",
  "      <string>support.constant.node, support.type.object.module.js</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#8839ef</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>implements as keyword</string>",
  "      <key>scope</key>",
  "      <string>storage.modifier.implements</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#8839ef</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>Builtin types</string>",
  "      <key>scope</key>",
  "      <string>constant.language.null.js, constant.language.null.ts, constant.language.undefined.js, constant.language.undefined.ts, support.type.builtin.ts</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#8839ef</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>variable.parameter.generic</string>",
  "      <key>scope</key>",
  "      <string>variable.parameter.generic</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#df8e1d</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>Arrow functions</string>",
  "      <key>scope</key>",
  "      <string>keyword.declaration.function.arrow.js, storage.type.function.arrow.ts</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#179299</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>Decorator punctuations (decorators inherit from blue functions, instead of styleguide peach)</string>",
  "      <key>scope</key>",
  "      <string>punctuation.decorator.ts</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#1e66f5</string>",
  "        <key>fontStyle</key>",
  "        <string>italic</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>Extra JS/TS keywords</string>",
  "      <key>scope</key>",
  "      <string>keyword.operator.expression.in.js, keyword.operator.expression.in.ts, keyword.operator.expression.infer.ts, keyword.operator.expression.instanceof.js, keyword.operator.expression.instanceof.ts, keyword.operator.expression.is, keyword.operator.expression.keyof.ts, keyword.operator.expression.of.js, keyword.operator.expression.of.ts, keyword.operator.expression.typeof.ts</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#8839ef</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>Julia macros</string>",
  "      <key>scope</key>",
  "      <string>support.function.macro.julia</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#179299</string>",
  "        <key>fontStyle</key>",
  "        <string>italic</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>Julia language constants (true, false)</string>",
  "      <key>scope</key>",
  "      <string>constant.language.julia</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#fe640b</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>Julia other constants (these seem to be arguments inside arrays)</string>",
  "      <key>scope</key>",
  "      <string>constant.other.symbol.julia</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#e64553</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>LaTeX preamble</string>",
  "      <key>scope</key>",
  "      <string>text.tex keyword.control.preamble</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#179299</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>LaTeX be functions</string>",
  "      <key>scope</key>",
  "      <string>text.tex support.function.be</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#04a5e5</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>LaTeX math</string>",
  "      <key>scope</key>",
  "      <string>constant.other.general.math.tex</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#dd7878</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>Liquid Builtin Objects &amp; User Defined Variables</string>",
  "      <key>scope</key>",
  "      <string>variable.language.liquid</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#ea76cb</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>Lua docstring keywords</string>",
  "      <key>scope</key>",
  "      <string>comment.line.double-dash.documentation.lua storage.type.annotation.lua</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#8839ef</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>Lua docstring variables</string>",
  "      <key>scope</key>",
  "      <string>comment.line.double-dash.documentation.lua entity.name.variable.lua, comment.line.double-dash.documentation.lua variable.lua</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#4c4f69</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>heading.1.markdown punctuation.definition.heading.markdown, heading.1.markdown, heading.1.quarto punctuation.definition.heading.quarto, heading.1.quarto, markup.heading.atx.1.mdx, markup.heading.atx.1.mdx punctuation.definition.heading.mdx, markup.heading.setext.1.markdown, markup.heading.heading-0.asciidoc</string>",
  "      <key>scope</key>",
  "      <string>heading.1.markdown punctuation.definition.heading.markdown, heading.1.markdown, heading.1.quarto punctuation.definition.heading.quarto, heading.1.quarto, markup.heading.atx.1.mdx, markup.heading.atx.1.mdx punctuation.definition.heading.mdx, markup.heading.setext.1.markdown, markup.heading.heading-0.asciidoc</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#d20f39</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>heading.2.markdown punctuation.definition.heading.markdown, heading.2.markdown, heading.2.quarto punctuation.definition.heading.quarto, heading.2.quarto, markup.heading.atx.2.mdx, markup.heading.atx.2.mdx punctuation.definition.heading.mdx, markup.heading.setext.2.markdown, markup.heading.heading-1.asciidoc</string>",
  "      <key>scope</key>",
  "      <string>heading.2.markdown punctuation.definition.heading.markdown, heading.2.markdown, heading.2.quarto punctuation.definition.heading.quarto, heading.2.quarto, markup.heading.atx.2.mdx, markup.heading.atx.2.mdx punctuation.definition.heading.mdx, markup.heading.setext.2.markdown, markup.heading.heading-1.asciidoc</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#fe640b</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>heading.3.markdown punctuation.definition.heading.markdown, heading.3.markdown, heading.3.quarto punctuation.definition.heading.quarto, heading.3.quarto, markup.heading.atx.3.mdx, markup.heading.atx.3.mdx punctuation.definition.heading.mdx, markup.heading.heading-2.asciidoc</string>",
  "      <key>scope</key>",
  "      <string>heading.3.markdown punctuation.definition.heading.markdown, heading.3.markdown, heading.3.quarto punctuation.definition.heading.quarto, heading.3.quarto, markup.heading.atx.3.mdx, markup.heading.atx.3.mdx punctuation.definition.heading.mdx, markup.heading.heading-2.asciidoc</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#df8e1d</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>heading.4.markdown punctuation.definition.heading.markdown, heading.4.markdown, heading.4.quarto punctuation.definition.heading.quarto, heading.4.quarto, markup.heading.atx.4.mdx, markup.heading.atx.4.mdx punctuation.definition.heading.mdx, markup.heading.heading-3.asciidoc</string>",
  "      <key>scope</key>",
  "      <string>heading.4.markdown punctuation.definition.heading.markdown, heading.4.markdown, heading.4.quarto punctuation.definition.heading.quarto, heading.4.quarto, markup.heading.atx.4.mdx, markup.heading.atx.4.mdx punctuation.definition.heading.mdx, markup.heading.heading-3.asciidoc</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#40a02b</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>heading.5.markdown punctuation.definition.heading.markdown, heading.5.markdown, heading.5.quarto punctuation.definition.heading.quarto, heading.5.quarto, markup.heading.atx.5.mdx, markup.heading.atx.5.mdx punctuation.definition.heading.mdx, markup.heading.heading-4.asciidoc</string>",
  "      <key>scope</key>",
  "      <string>heading.5.markdown punctuation.definition.heading.markdown, heading.5.markdown, heading.5.quarto punctuation.definition.heading.quarto, heading.5.quarto, markup.heading.atx.5.mdx, markup.heading.atx.5.mdx punctuation.definition.heading.mdx, markup.heading.heading-4.asciidoc</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#209fb5</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>heading.6.markdown punctuation.definition.heading.markdown, heading.6.markdown, heading.6.quarto punctuation.definition.heading.quarto, heading.6.quarto, markup.heading.atx.6.mdx, markup.heading.atx.6.mdx punctuation.definition.heading.mdx, markup.heading.heading-5.asciidoc</string>",
  "      <key>scope</key>",
  "      <string>heading.6.markdown punctuation.definition.heading.markdown, heading.6.markdown, heading.6.quarto punctuation.definition.heading.quarto, heading.6.quarto, markup.heading.atx.6.mdx, markup.heading.atx.6.mdx punctuation.definition.heading.mdx, markup.heading.heading-5.asciidoc</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#7287fd</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>markup.bold</string>",
  "      <key>scope</key>",
  "      <string>markup.bold</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#d20f39</string>",
  "        <key>fontStyle</key>",
  "        <string>bold</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>markup.italic</string>",
  "      <key>scope</key>",
  "      <string>markup.italic</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#d20f39</string>",
  "        <key>fontStyle</key>",
  "        <string>italic</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>markup.strikethrough</string>",
  "      <key>scope</key>",
  "      <string>markup.strikethrough</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#6c6f85</string>",
  "        <key>fontStyle</key>",
  "        <string>strikethrough</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>Markdown auto links</string>",
  "      <key>scope</key>",
  "      <string>punctuation.definition.link, markup.underline.link</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#1e66f5</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>Markdown links</string>",
  "      <key>scope</key>",
  "      <string>text.html.markdown punctuation.definition.link.title, text.html.quarto punctuation.definition.link.title, string.other.link.title.markdown, string.other.link.title.quarto, markup.link, punctuation.definition.constant.markdown, punctuation.definition.constant.quarto, constant.other.reference.link.markdown, constant.other.reference.link.quarto, markup.substitution.attribute-reference</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#7287fd</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>Markdown code spans</string>",
  "      <key>scope</key>",
  "      <string>punctuation.definition.raw.markdown, punctuation.definition.raw.quarto, markup.inline.raw.string.markdown, markup.inline.raw.string.quarto, markup.raw.block.markdown, markup.raw.block.quarto</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#40a02b</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>Markdown triple backtick language identifier</string>",
  "      <key>scope</key>",
  "      <string>fenced_code.block.language</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#04a5e5</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>Markdown triple backticks</string>",
  "      <key>scope</key>",
  "      <string>markup.fenced_code.block punctuation.definition, markup.raw support.asciidoc</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#7c7f93</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>Markdown quotes</string>",
  "      <key>scope</key>",
  "      <string>markup.quote, punctuation.definition.quote.begin</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#ea76cb</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>Markdown separators</string>",
  "      <key>scope</key>",
  "      <string>meta.separator.markdown</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#179299</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>Markdown list bullets</string>",
  "      <key>scope</key>",
  "      <string>punctuation.definition.list.begin.markdown, punctuation.definition.list.begin.quarto, markup.list.bullet</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#179299</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>Quarto headings</string>",
  "      <key>scope</key>",
  "      <string>markup.heading.quarto</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>fontStyle</key>",
  "        <string>bold</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>Nix attribute names</string>",
  "      <key>scope</key>",
  "      <string>entity.other.attribute-name.multipart.nix, entity.other.attribute-name.single.nix</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#1e66f5</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>Nix parameter names</string>",
  "      <key>scope</key>",
  "      <string>variable.parameter.name.nix</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#4c4f69</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>Nix interpolated parameter names</string>",
  "      <key>scope</key>",
  "      <string>meta.embedded variable.parameter.name.nix</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#7287fd</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>Nix paths</string>",
  "      <key>scope</key>",
  "      <string>string.unquoted.path.nix</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#ea76cb</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>PHP Attributes</string>",
  "      <key>scope</key>",
  "      <string>support.attribute.builtin, meta.attribute.php</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#df8e1d</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>PHP Parameters (needed for the leading dollar sign)</string>",
  "      <key>scope</key>",
  "      <string>meta.function.parameters.php punctuation.definition.variable.php</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#e64553</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>PHP Constants (null, __FILE__, etc.)</string>",
  "      <key>scope</key>",
  "      <string>constant.language.php</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#8839ef</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>PHP functions</string>",
  "      <key>scope</key>",
  "      <string>text.html.php support.function</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#04a5e5</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>PHPdoc keywords</string>",
  "      <key>scope</key>",
  "      <string>keyword.other.phpdoc.php</string>",
  "      <key>settings</key>",
  "      <dict>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>Python argument functions reset to text, otherwise they inherit blue from function-call</string>",
  "      <key>scope</key>",
  "      <string>support.variable.magic.python, meta.function-call.arguments.python</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#4c4f69</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>Python double underscore functions</string>",
  "      <key>scope</key>",
  "      <string>support.function.magic.python</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#04a5e5</string>",
  "        <key>fontStyle</key>",
  "        <string>italic</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>Python `self` keyword</string>",
  "      <key>scope</key>",
  "      <string>variable.parameter.function.language.special.self.python, variable.language.special.self.python</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#d20f39</string>",
  "        <key>fontStyle</key>",
  "        <string>italic</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>python keyword flow/logical (for ... in)</string>",
  "      <key>scope</key>",
  "      <string>keyword.control.flow.python, keyword.operator.logical.python</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#8839ef</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>python storage type</string>",
  "      <key>scope</key>",
  "      <string>storage.type.function.python</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#8839ef</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>python function support</string>",
  "      <key>scope</key>",
  "      <string>support.token.decorator.python, meta.function.decorator.identifier.python</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#04a5e5</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>python function calls</string>",
  "      <key>scope</key>",
  "      <string>meta.function-call.python</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#1e66f5</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>python function decorators</string>",
  "      <key>scope</key>",
  "      <string>entity.name.function.decorator.python, punctuation.definition.decorator.python</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#fe640b</string>",
  "        <key>fontStyle</key>",
  "        <string>italic</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>python placeholder reset to normal string</string>",
  "      <key>scope</key>",
  "      <string>constant.character.format.placeholder.other.python</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#ea76cb</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>Python exception &amp; builtins such as exit()</string>",
  "      <key>scope</key>",
  "      <string>support.type.exception.python, support.function.builtin.python</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#fe640b</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>entity.name.type</string>",
  "      <key>scope</key>",
  "      <string>support.type.python</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#8839ef</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>python constants (True/False)</string>",
  "      <key>scope</key>",
  "      <string>constant.language.python</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#fe640b</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>Arguments accessed later in the function body</string>",
  "      <key>scope</key>",
  "      <string>meta.indexed-name.python, meta.item-access.python</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#e64553</string>",
  "        <key>fontStyle</key>",
  "        <string>italic</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>Python f-strings/binary/unicode storage types</string>",
  "      <key>scope</key>",
  "      <string>storage.type.string.python</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#40a02b</string>",
  "        <key>fontStyle</key>",
  "        <string>italic</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>Python type hints</string>",
  "      <key>scope</key>",
  "      <string>meta.function.parameters.python</string>",
  "      <key>settings</key>",
  "      <dict>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>Regex string begin/end in JS/TS</string>",
  "      <key>scope</key>",
  "      <string>string.regexp punctuation.definition.string.begin, string.regexp punctuation.definition.string.end</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#ea76cb</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>Regex anchors (^, $)</string>",
  "      <key>scope</key>",
  "      <string>keyword.control.anchor.regexp</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#8839ef</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>Regex regular string match</string>",
  "      <key>scope</key>",
  "      <string>string.regexp.ts</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#4c4f69</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>Regex group parenthesis &amp; backreference (\\1, \\2, \\3, ...)</string>",
  "      <key>scope</key>",
  "      <string>punctuation.definition.group.regexp, keyword.other.back-reference.regexp</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#40a02b</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>Regex character class []</string>",
  "      <key>scope</key>",
  "      <string>punctuation.definition.character-class.regexp</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#df8e1d</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>Regex character classes (\\d, \\w, \\s)</string>",
  "      <key>scope</key>",
  "      <string>constant.other.character-class.regexp</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#ea76cb</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>Regex range</string>",
  "      <key>scope</key>",
  "      <string>constant.other.character-class.range.regexp</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#dc8a78</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>Regex quantifier</string>",
  "      <key>scope</key>",
  "      <string>keyword.operator.quantifier.regexp</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#179299</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>Regex constant/numeric</string>",
  "      <key>scope</key>",
  "      <string>constant.character.numeric.regexp</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#fe640b</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>Regex lookaheads, negative lookaheads, lookbehinds, negative lookbehinds</string>",
  "      <key>scope</key>",
  "      <string>punctuation.definition.group.no-capture.regexp, meta.assertion.look-ahead.regexp, meta.assertion.negative-look-ahead.regexp</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#1e66f5</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>Rust attribute</string>",
  "      <key>scope</key>",
  "      <string>meta.annotation.rust, meta.annotation.rust punctuation, meta.attribute.rust, punctuation.definition.attribute.rust</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#df8e1d</string>",
  "        <key>fontStyle</key>",
  "        <string>italic</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>Rust attribute strings</string>",
  "      <key>scope</key>",
  "      <string>meta.attribute.rust string.quoted.double.rust, meta.attribute.rust string.quoted.single.char.rust</string>",
  "      <key>settings</key>",
  "      <dict>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>Rust keyword</string>",
  "      <key>scope</key>",
  "      <string>entity.name.function.macro.rules.rust, storage.type.module.rust, storage.modifier.rust, storage.type.struct.rust, storage.type.enum.rust, storage.type.trait.rust, storage.type.union.rust, storage.type.impl.rust, storage.type.rust, storage.type.function.rust, storage.type.type.rust</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#8839ef</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>Rust u/i32, u/i64, etc.</string>",
  "      <key>scope</key>",
  "      <string>entity.name.type.numeric.rust</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#8839ef</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>Rust generic</string>",
  "      <key>scope</key>",
  "      <string>meta.generic.rust</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#fe640b</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>Rust impl</string>",
  "      <key>scope</key>",
  "      <string>entity.name.impl.rust</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#df8e1d</string>",
  "        <key>fontStyle</key>",
  "        <string>italic</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>Rust module</string>",
  "      <key>scope</key>",
  "      <string>entity.name.module.rust</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#fe640b</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>Rust trait</string>",
  "      <key>scope</key>",
  "      <string>entity.name.trait.rust</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#df8e1d</string>",
  "        <key>fontStyle</key>",
  "        <string>italic</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>Rust struct</string>",
  "      <key>scope</key>",
  "      <string>storage.type.source.rust</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#df8e1d</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>Rust union</string>",
  "      <key>scope</key>",
  "      <string>entity.name.union.rust</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#df8e1d</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>Rust enum member</string>",
  "      <key>scope</key>",
  "      <string>meta.enum.rust storage.type.source.rust</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#179299</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>Rust macro</string>",
  "      <key>scope</key>",
  "      <string>support.macro.rust, meta.macro.rust support.function.rust, entity.name.function.macro.rust</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#1e66f5</string>",
  "        <key>fontStyle</key>",
  "        <string>italic</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>Rust lifetime</string>",
  "      <key>scope</key>",
  "      <string>storage.modifier.lifetime.rust, entity.name.type.lifetime</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#1e66f5</string>",
  "        <key>fontStyle</key>",
  "        <string>italic</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>Rust string formatting</string>",
  "      <key>scope</key>",
  "      <string>string.quoted.double.rust constant.other.placeholder.rust</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#ea76cb</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>Rust return type generic</string>",
  "      <key>scope</key>",
  "      <string>meta.function.return-type.rust meta.generic.rust storage.type.rust</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#4c4f69</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>Rust functions</string>",
  "      <key>scope</key>",
  "      <string>meta.function.call.rust</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#1e66f5</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>Rust angle brackets</string>",
  "      <key>scope</key>",
  "      <string>punctuation.brackets.angle.rust</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#04a5e5</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>Rust constants</string>",
  "      <key>scope</key>",
  "      <string>constant.other.caps.rust</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#fe640b</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>Rust function parameters</string>",
  "      <key>scope</key>",
  "      <string>meta.function.definition.rust variable.other.rust</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#e64553</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>Rust closure variables</string>",
  "      <key>scope</key>",
  "      <string>meta.function.call.rust variable.other.rust</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#4c4f69</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>Rust self</string>",
  "      <key>scope</key>",
  "      <string>variable.language.self.rust</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#d20f39</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>Rust metavariable names</string>",
  "      <key>scope</key>",
  "      <string>variable.other.metavariable.name.rust, meta.macro.metavariable.rust keyword.operator.macro.dollar.rust</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#ea76cb</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>Shell shebang</string>",
  "      <key>scope</key>",
  "      <string>comment.line.shebang, comment.line.shebang punctuation.definition.comment, comment.line.shebang, punctuation.definition.comment.shebang.shell, meta.shebang.shell</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#ea76cb</string>",
  "        <key>fontStyle</key>",
  "        <string>italic</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>Shell shebang command</string>",
  "      <key>scope</key>",
  "      <string>comment.line.shebang constant.language</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#179299</string>",
  "        <key>fontStyle</key>",
  "        <string>italic</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>Shell interpolated command</string>",
  "      <key>scope</key>",
  "      <string>meta.function-call.arguments.shell punctuation.definition.variable.shell, meta.function-call.arguments.shell punctuation.section.interpolation, meta.function-call.arguments.shell punctuation.definition.variable.shell, meta.function-call.arguments.shell punctuation.section.interpolation</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#d20f39</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>Shell interpolated command variable</string>",
  "      <key>scope</key>",
  "      <string>meta.string meta.interpolation.parameter.shell variable.other.readwrite</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#fe640b</string>",
  "        <key>fontStyle</key>",
  "        <string>italic</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>source.shell punctuation.section.interpolation, punctuation.definition.evaluation.backticks.shell</string>",
  "      <key>scope</key>",
  "      <string>source.shell punctuation.section.interpolation, punctuation.definition.evaluation.backticks.shell</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#179299</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>Shell EOF</string>",
  "      <key>scope</key>",
  "      <string>entity.name.tag.heredoc.shell</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#8839ef</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>Shell quoted variable</string>",
  "      <key>scope</key>",
  "      <string>string.quoted.double.shell variable.other.normal.shell</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#4c4f69</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>markup.heading.typst</string>",
  "      <key>scope</key>",
  "      <string>markup.heading.typst</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#d20f39</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>JSON Keys</string>",
  "      <key>scope</key>",
  "      <string>source.json meta.mapping.key string</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#1e66f5</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>JSON key surrounding quotes</string>",
  "      <key>scope</key>",
  "      <string>source.json meta.mapping.key punctuation.definition.string.begin, source.json meta.mapping.key punctuation.definition.string.end</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#7c7f93</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>markup.heading.synopsis.man, markup.heading.title.man, markup.heading.other.man, markup.heading.env.man</string>",
  "      <key>scope</key>",
  "      <string>markup.heading.synopsis.man, markup.heading.title.man, markup.heading.other.man, markup.heading.env.man</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#8839ef</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>markup.heading.commands.man</string>",
  "      <key>scope</key>",
  "      <string>markup.heading.commands.man</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#1e66f5</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>markup.heading.env.man</string>",
  "      <key>scope</key>",
  "      <string>markup.heading.env.man</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#ea76cb</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>Man page options</string>",
  "      <key>scope</key>",
  "      <string>entity.name</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#179299</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>markup.heading.1.markdown</string>",
  "      <key>scope</key>",
  "      <string>markup.heading.1.markdown</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#d20f39</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>markup.heading.2.markdown</string>",
  "      <key>scope</key>",
  "      <string>markup.heading.2.markdown</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#fe640b</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>markup.heading.markdown</string>",
  "      <key>scope</key>",
  "      <string>markup.heading.markdown</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#df8e1d</string>",
  "      </dict>",
  "    </dict>",
  "  </array>",
  "</dict>",
  "</plist>",
).join("\n"))

#let _mode = sys.inputs.at("calepin-mode", default: "render")
#let _auto-label-index = state("calepin-auto-label-index", 1)
#let _auto-inline-label-index = state("calepin-auto-inline-label-index", 1)

// Website pages index, provided by `calepin compile` during website builds.
#let _pages-index-path = sys.inputs.at("calepin-pages", default: "")
#let _current-page-href = sys.inputs.at("calepin-current-href", default: "")

// Relative URL prefix from the current page back to the site root.
#let _site-root-prefix() = {
  let depth = _current-page-href.split("/").filter(part => part != "").len() - 1
  if depth <= 0 { "" } else { "../" * depth }
}

// Returns one entry per page of the website: a dictionary with `path` (source
// file), `href` (link to the page, relative to the current page), `title`
// (resolved display title), `pdf` (link to the PDF twin, or none), and `meta`
// (the page's raw `<website-metadata>` dictionary, verbatim). Returns an
// empty array outside website builds.
#let _prefix-page-entry(entry, prefix) = {
  let entry = entry
  if type(entry.at("href", default: none)) == str {
    entry.insert("href", prefix + entry.href)
  }
  if type(entry.at("pdf", default: none)) == str {
    entry.insert("pdf", prefix + entry.pdf)
  }
  if type(entry.at("translations", default: none)) == dictionary {
    let translations = (:)
    for (language, href) in entry.translations.pairs() {
      translations.insert(language, if type(href) == str { prefix + href } else { href })
    }
    entry.insert("translations", translations)
  }
  entry
}

#let pages() = {
  if _pages-index-path == "" { return () }
  let prefix = _site-root-prefix()
  json(_pages-index-path).map(entry => _prefix-page-entry(entry, prefix))
}

#let _base-options = (
  echo: true,
  eval: true,
  results: "render",
  warning: true,
  message: true,
  error: false,
  placeholder: auto,
  "fig-device-format": "svg",
  "fig-device-dpi": 150,
  "fig-device-width": 6,
  "fig-device-height": auto,
  "fig-device-aspect": 0.618,
  "fig-width": 70%,
  "fig-height": auto,
  "fig-align": center,
  "fig-responsive": true,
  "fig-link": auto,
  "fig-caption": none,
  "fig-cap-location": auto,
  "fig-alt-text": none,
  "fig-subcaptions": none,
  "fig-layout-columns": auto,
  "fig-layout-rows": auto,
  kind: auto,
  "fenced-chunks": true,
)
#let _setup-defaults = state("calepin-setup-defaults", (default: _base-options))

#let _call-extra-defaults = (
  label: none,
  inline-output: false,
  auto-label-prefix: "chunk",
  auto-label-state: _auto-label-index,
)

#let _auto-call-defaults(defaults) = {
  let out = (:)
  for key in defaults.keys() {
    out.insert(key, auto)
  }
  out.insert("fig-link", none)
  out.insert("fig-caption", none)
  out.insert("fig-alt-text", none)
  out.insert("fig-subcaptions", none)
  out + _call-extra-defaults
}

#let _call-defaults = _auto-call-defaults(_base-options)

#let _disable-raw-chunk-transforms = state("calepin-disable-raw-chunk-transforms", false)

#let _raw-node(body) = {
  if body.has("text") {
    return body
  }
  if body.has("children") {
    let candidates = body.children.filter(child => child.has("text"))
    if candidates.len() == 1 {
      return candidates.at(0)
    }
  }
  panic("calepin chunks must contain exactly one raw code element")
}

#let _raw-text(body) = _raw-node(body).text

#let _sync-auto-label-counter(auto-label-state, label) = {
  if label.starts-with("chunk-") {
    let suffix = label.slice(6)
    let is-int = suffix.matches(regex("^[0-9]+$")) != ()
    if is-int {
      let next = int(suffix) + 1
      auto-label-state.update(n => if next > n { next } else { n })
    }
  }
}

// Accept `label` as none | str | array of str. Returns the internal id (used
// for results lookup + artifact filenames) and the raw label-name list.
#let _derive-label(label-opt, generated-prefix, counter-value) = {
  if label-opt == none {
    (id: generated-prefix + "-" + str(counter-value), names: (), generated: true)
  } else if type(label-opt) == str {
    (id: label-opt, names: (label-opt,), generated: false)
  } else if type(label-opt) == array {
    if label-opt.len() == 0 { panic("calepin.chunk: label list must not be empty") }
    for entry in label-opt {
      if type(entry) != str { panic("calepin.chunk: label entries must be strings") }
    }
    (id: label-opt.first(), names: label-opt, generated: false)
  } else {
    panic("calepin.chunk: label must be a string or an array of strings")
  }
}

#let _select-representation(data) = {
  for mime in ("image/svg+xml", "image/png", "text/x-typst", "text/plain", "application/json") {
    let value = data.at(mime, default: none)
    if value != none {
      return (mime: mime, value: value)
    }
  }
  none
}

#let _artifact-path(value) = {
  if type(value) == dictionary {
    value.at("path")
  } else {
    value
  }
}

#let _resolve-asset-href(path) = {
  let base = sys.inputs.at("calepin-assets", default: "")
  if base != "" and path.starts-with("/") {
    base + path
  } else {
    path
  }
}

#let _html-target() = sys.inputs.at("calepin-target", default: "paged") == "html"

#let _attach-label(content, id) = [
  #content #label(id)
]

#let _attach-labels(content, ids) = {
  let out = content
  for id in ids {
    out = [#out #label(id)]
  }
  out
}

#let _crossref-labels-for(chunk, kind) = {
  let labels = ()
  for entry in chunk.at("crossref-labels", default: ()) {
    if entry.at("kind", default: "") == kind {
      labels.push(entry.at("name"))
    }
  }
  labels
}
#let _figure-caption(fig-caption, fig-cap-location) = {
  if fig-caption == none {
    none
  } else if fig-cap-location == auto or fig-cap-location == none {
    fig-caption
  } else {
    figure.caption(position: fig-cap-location)[#fig-caption]
  }
}

#let _css-size(value) = {
  if value == none or value == auto {
    none
  } else if type(value) == str {
    value
  } else {
    repr(value)
  }
}

#let _css-decl(property, value) = {
  let size = _css-size(value)
  if size == none or size == "" {
    ""
  } else {
    property + ": " + size + ";"
  }
}

#let _append-css(base, next) = {
  if next == "" {
    base
  } else if base == "" {
    next
  } else {
    base + " " + next
  }
}

#let _normalize-display-align(fig-align) = {
  if fig-align == "left" {
    left
  } else if fig-align == "start" {
    start
  } else if fig-align == "right" {
    right
  } else if fig-align == "end" {
    end
  } else if fig-align == "center" {
    center
  } else {
    fig-align
  }
}

#let _html-image-align-style(fig-align) = {
  let fig-align = _normalize-display-align(fig-align)
  if fig-align == left or fig-align == start {
    "margin-inline: 0 auto;"
  } else if fig-align == right or fig-align == end {
    "margin-inline: auto 0;"
  } else {
    "margin-inline: auto;"
  }
}

#let _html-block-align-style(fig-align) = {
  let fig-align = _normalize-display-align(fig-align)
  if fig-align == left or fig-align == start {
    "text-align: left;"
  } else if fig-align == right or fig-align == end {
    "text-align: right;"
  } else if fig-align == center {
    "text-align: center;"
  } else {
    ""
  }
}

#let _html-image-style(width, height, responsive, fig-align) = {
  let base = _append-css("display: block;", _html-image-align-style(fig-align))
  let with-width = _append-css(base, _css-decl("width", width))
  let with-height = _append-css(with-width, _css-decl("height", height))
  if responsive == true {
    _append-css(with-height, "max-width: 100%;")
  } else {
    with-height
  }
}

#let _html-image(path, width, height, responsive, fig-align, alt) = {
  let style = _html-image-style(width, height, responsive, fig-align)
  if style == "" {
    std.html.elem("img", attrs: (src: path, alt: alt))
  } else {
    std.html.elem("img", attrs: (src: path, alt: alt, style: style))
  }
}

#let _html-captioned-image(path, height, alt) = {
  let style = _append-css(_append-css("display: block;", "width: 100%;"), _css-decl("height", height))
  std.html.elem("img", attrs: (src: path, alt: alt, style: style))
}

#let _html-figure-style(width, responsive, fig-align) = {
  let with-width = _css-decl("width", width)
  let with-responsive = if responsive == true {
    _append-css(with-width, "max-width: 100%;")
  } else {
    with-width
  }
  _append-css(with-responsive, _html-image-align-style(fig-align))
}

// A labeled figure must stay a native `figure` so `@label` cross-references
// resolve, and a native figure cannot carry the display-width style itself.
// Wrap it in a styled block that applies the same width/responsive/alignment as
// an unlabeled captioned figure, so both honor `fig-width`.
#let _wrap-html-figure-width(content, width, responsive, fig-align) = {
  let style = _html-figure-style(width, responsive, fig-align)
  if style == "" {
    content
  } else {
    std.html.elem("div", attrs: (style: style))[#content]
  }
}

#let _html-captioned-figure(
  img,
  width,
  responsive,
  fig-align,
  fig-caption,
  fig-cap-location,
) = {
  let style = _html-figure-style(width, responsive, fig-align)
  let attrs = if style == "" { (:) } else { (style: style) }
  let caption = std.html.elem("figcaption")[#context [Figure #counter(figure).display(): #fig-caption]]
  let content = if fig-cap-location == top {
    [#caption #img]
  } else {
    [#img #caption]
  }
  [
    #counter(figure).step()
    #std.html.elem("figure", attrs: attrs)[#content]
  ]
}

#let _finalize-figure-display(content, fig-align, fig-link) = {
  let fig-align = _normalize-display-align(fig-align)
  let linked = if fig-link == none or fig-link == auto {
    content
  } else {
    link(fig-link)[#content]
  }
  if _html-target() {
    let style = _html-block-align-style(fig-align)
    if style == "" {
      return linked
    }
    return std.html.elem("div", attrs: (style: style))[#linked]
  }
  if fig-align == none or fig-align == auto {
    linked
  } else {
    align(fig-align)[#linked]
  }
}

#let _paged-result-options(options) = {
  let out = (:)
  if "fig-align" in options {
    out.insert("fig-align", options.at("fig-align"))
  }
  out
}

#let _merge-result-options(opts, chunk) = {
  let options = chunk.at("options", default: (:))
  if _html-target() {
    opts + options
  } else {
    opts + _paged-result-options(options)
  }
}

#let _block-lang-label(lang) = {
  if lang == none {
    ""
  } else if lang == "r" {
    "R"
  } else {
    lang
  }
}

#let _raw-block(value, lang: none, theme: auto) = {
  show raw.where(block: true): set text(size: 1em)
  raw(value, block: true, lang: lang, theme: theme)
}

#let _html-themed-raw-block(it) = {
  if _html-target() {
    let lang = if it.has("lang") { it.lang } else { none }
    _raw-block(it.text, lang: lang, theme: _input-syntax-theme)
  } else {
    it
  }
}

#let code-block(
  body,
  fill: rgb("#f7f7f5"),
  stroke: 0.5pt + rgb("#d8d8d2"),
  radius: 2pt,
  inset: (x: 0.65em, y: 0.45em),
  text-fill: rgb("#1f2933"),
  plain: false,
) = {
  let content = if plain {
    body
  } else {
    text(fill: text-fill)[#body]
  }
  block(
    width: 100%,
    fill: fill,
    stroke: stroke,
    radius: radius,
    inset: inset,
  )[
    #content
  ]
}

#let _input-block(code, lang: none) = {
  if _html-target() {
    std.html.elem("div", attrs: (
      class: "sourceCode",
      "data-lang": _block-lang-label(lang),
    ))[
      #_raw-block(code, lang: lang, theme: _input-syntax-theme)
    ]
  } else {
    code-block(
      fill: rgb("#f7f7f5"),
      stroke: 0.5pt + rgb("#d8d8d2"),
      radius: 2pt,
      inset: (x: 0.65em, y: 0.45em),
    )[
      #text(fill: rgb("#1f2933"))[
        #_raw-block(code, lang: lang, theme: _paged-syntax-theme)
      ]
    ]
  }
}

#let _output-block(output, stream: "stdout") = {
  if _html-target() {
    let class = if stream == "stderr" {
      "cell-output cell-output-stderr"
    } else {
      "cell-output cell-output-stdout"
    }
    std.html.elem("div", attrs: (class: class))[
      #_raw-block(output, theme: _output-syntax-theme)
    ]
  } else {
    let fill = if stream == "stderr" {
      rgb("#fffaf7")
    } else {
      rgb("#fbfbfa")
    }
    let stroke = if stream == "stderr" {
      (
        rest: 0.5pt + rgb("#e2c7ba"),
        left: 1.5pt + rgb("#c48672"),
      )
    } else {
      (
        rest: 0.5pt + rgb("#ddddda"),
        left: 1.5pt + rgb("#cfcfc8"),
      )
    }
    code-block(
      fill: fill,
      stroke: stroke,
      radius: 2pt,
      inset: (x: 0.65em, y: 0.4em),
      plain: true,
    )[
      #if stream == "stderr" {
        text(fill: rgb("#5f3328"))[
          #_raw-block(output, theme: _paged-syntax-theme)
        ]
      } else {
        _raw-block(output, theme: _paged-syntax-theme)
      }
    ]
  }
}

#let _display-selection(item, opts) = {
  let data = item.at("data", default: (:))
  _select-representation(data)
}

#let _is-image-mime(mime) = mime == "image/svg+xml" or mime == "image/png"

#let _is-image-display-item(item, opts) = {
  let item-type = item.at("type", default: "")
  if item-type != "display" and item-type != "result" {
    return false
  }
  let selected = _display-selection(item, opts)
  selected != none and _is-image-mime(selected.mime)
}

#let _fr-tracks(count) = {
  let tracks = ()
  for _ in range(count) {
    tracks.push(1fr)
  }
  tracks
}

#let _track-list(value) = {
  if value == auto or value == none {
    auto
  } else if type(value) == int {
    _fr-tracks(value)
  } else {
    value
  }
}

#let _auto-grid-columns(count, fig-layout-rows) = {
  if type(fig-layout-rows) == int and fig-layout-rows > 0 {
    return _fr-tracks(calc.ceil(count / fig-layout-rows))
  }
  if count <= 1 {
    (1fr,)
  } else if count <= 4 {
    (1fr, 1fr)
  } else {
    (1fr, 1fr, 1fr)
  }
}

#let _grid-columns(count, fig-layout-columns, fig-layout-rows) = {
  let columns = _track-list(fig-layout-columns)
  if columns == auto {
    _auto-grid-columns(count, fig-layout-rows)
  } else {
    columns
  }
}

#let _css-track(value) = {
  if value == auto {
    "auto"
  } else if type(value) == str {
    value
  } else {
    repr(value)
  }
}

#let _css-track-template(value) = {
  if value == auto or value == none {
    none
  } else if type(value) == array {
    let tracks = ()
    for track in value {
      tracks.push(_css-track(track))
    }
    tracks.join(" ")
  } else {
    _css-track(value)
  }
}

#let _html-grid-style(columns, rows) = {
  let style = "display: grid; gap: 1em;"
  let column-template = _css-track-template(columns)
  if column-template != none {
    style = _append-css(style, "grid-template-columns: " + column-template + ";")
  }
  let row-template = _css-track-template(rows)
  if row-template != none {
    style = _append-css(style, "grid-template-rows: " + row-template + ";")
  }
  style
}

#let _html-grid-content(columns, rows, cells) = {
  let body = []
  for cell in cells {
    body += cell
  }
  std.html.elem("div", attrs: (
    class: "calepin-figure-grid",
    style: _html-grid-style(columns, rows),
  ))[#body]
}

#let _grid-content(columns, rows, cells) = {
  let rows = _track-list(rows)
  if _html-target() {
    _html-grid-content(columns, rows, cells)
  } else if rows == auto {
    grid(columns: columns, gutter: 1em, ..cells)
  } else {
    grid(columns: columns, rows: rows, gutter: 1em, ..cells)
  }
}

#let _caption-for-index(captions, index) = {
  if captions == none or captions == auto {
    none
  } else if type(captions) == array and index < captions.len() {
    captions.at(index)
  } else {
    none
  }
}

#let _grid-image(item, opts) = {
  let selected = _display-selection(item, opts)
  let value = selected.value
  let artifact-path = _artifact-path(value)
  let html-path = _resolve-asset-href(artifact-path)
  let fig-height = opts.at("fig-height")
  let fig-responsive = opts.at("fig-responsive")
  let fig-alt-text = opts.at("fig-alt-text")
  let alt = if fig-alt-text == none { "" } else { fig-alt-text }
  if _html-target() {
    _html-image(html-path, 100%, fig-height, fig-responsive, center, alt)
  } else {
    image(artifact-path, width: 100%, height: fig-height, alt: alt)
  }
}

#let _grid-cell(content, caption) = {
  if _html-target() and caption != none {
    std.html.elem("div", attrs: (style: "min-width: 0;"))[
      #content
      #std.html.elem("div", attrs: (style: "font-size: 0.85em; margin-top: 0.35em;"))[#caption]
    ]
  } else if _html-target() {
    std.html.elem("div", attrs: (style: "min-width: 0;"))[#content]
  } else if caption == none {
    content
  } else {
    stack(spacing: 0.35em, content, text(size: 0.85em)[#caption])
  }
}

#let _wrap-grid-display(content, width, responsive, align) = {
  if _html-target() {
    let style = _html-figure-style(width, responsive, align)
    if style == "" {
      std.html.elem("div")[#content]
    } else {
      std.html.elem("div", attrs: (style: style))[#content]
    }
  } else if width == none or width == auto {
    content
  } else {
    block(width: width)[#content]
  }
}

#let _render-image-grid(items, label, opts, fig-labels) = {
  let fig-width = opts.at("fig-width")
  let fig-align = opts.at("fig-align")
  let fig-responsive = opts.at("fig-responsive")
  let fig-link = opts.at("fig-link")
  let fig-caption = opts.at("fig-caption")
  let fig-cap-location = opts.at("fig-cap-location")
  let fig-subcaptions = opts.at("fig-subcaptions")
  let fig-layout-columns = opts.at("fig-layout-columns")
  let fig-layout-rows = opts.at("fig-layout-rows")

  let cells = ()
  for (index, item) in items.enumerate() {
    cells.push(_grid-cell(_grid-image(item, opts), _caption-for-index(fig-subcaptions, index)))
  }

  let columns = _grid-columns(items.len(), fig-layout-columns, fig-layout-rows)
  let content = _wrap-grid-display(
    _grid-content(columns, fig-layout-rows, cells),
    fig-width,
    fig-responsive,
    fig-align,
  )
  let rendered = if fig-caption != none or fig-labels.len() > 0 {
    let fig = figure(content, caption: _figure-caption(fig-caption, fig-cap-location))
    if fig-labels.len() > 0 {
      _attach-labels(fig, fig-labels)
    } else {
      _attach-label(fig, label)
    }
  } else {
    content
  }
  _finalize-figure-display(rendered, fig-align, fig-link)
}

#let _render-display-item(item, label, opts, fig-labels) = {
  let fig-width = opts.at("fig-width")
  let fig-height = opts.at("fig-height")
  let fig-align = opts.at("fig-align")
  let fig-responsive = opts.at("fig-responsive")
  let fig-link = opts.at("fig-link")
  let fig-caption = opts.at("fig-caption")
  let fig-cap-location = opts.at("fig-cap-location")
  let fig-alt-text = opts.at("fig-alt-text")
  let selected = _display-selection(item, opts)
  if selected == none {
    return none
  }
  let mime = selected.mime
  let value = selected.value
  if _is-image-mime(mime) {
    let artifact-path = _artifact-path(value)
    let html-path = _resolve-asset-href(artifact-path)
    let display-width = if fig-width == auto and fig-responsive == true { 100% } else { fig-width }
    let alt = if fig-alt-text == none { "" } else { fig-alt-text }
    if _html-target() and fig-caption != none {
      let img = _html-captioned-image(html-path, fig-height, alt)
      let fig = if fig-labels.len() > 0 {
        figure(img, caption: _figure-caption(fig-caption, fig-cap-location))
      } else {
        _html-captioned-figure(img, display-width, fig-responsive, fig-align, fig-caption, fig-cap-location)
      }
      let rendered = if fig-labels.len() > 0 {
        _wrap-html-figure-width(
          _attach-labels(fig, fig-labels),
          display-width,
          fig-responsive,
          fig-align,
        )
      } else {
        _attach-label(fig, label)
      }
      return _finalize-figure-display(rendered, none, fig-link)
    }
    let img = if _html-target() {
      _html-image(html-path, display-width, fig-height, fig-responsive, fig-align, alt)
    } else {
      image(
        artifact-path,
        width: display-width,
        height: fig-height,
        alt: alt,
      )
    }
    let rendered = if fig-caption != none or fig-labels.len() > 0 {
      let fig = figure(img, caption: _figure-caption(fig-caption, fig-cap-location))
      if fig-labels.len() > 0 {
        _attach-labels(fig, fig-labels)
      } else {
        _attach-label(fig, label)
      }
    } else {
      img
    }
    _finalize-figure-display(rendered, fig-align, fig-link)
  } else if mime == "text/x-typst" {
    if type(value) == dictionary and value.at("path", default: none) != none {
      eval(read(_artifact-path(value), encoding: "utf8"), mode: "markup")
    } else {
      eval(value, mode: "markup")
    }
  } else if mime == "application/json" {
    _output-block(repr(value))
  } else {
    _output-block(str(value))
  }
}

#let _render-item(item, label, opts, fig-labels) = {
  let results-mode = opts.at("results")
  let inline-output = opts.at("inline-output")
  let warning = opts.at("warning")
  let message = opts.at("message")

  let item-type = item.at("type", default: "")
  if item-type == "stream" {
    let text = item.at("text", default: "")
    if results-mode == "hide" {
      none
    } else if results-mode == "typst" {
      eval(text, mode: "markup")
    } else if inline-output {
      text
    } else {
      _output-block(text)
    }
  } else if item-type == "diagnostic" {
    let level = item.at("level", default: "")
    if (level == "warning" and warning != true) or (level == "message" and message != true) {
      none
    } else {
      _output-block(item.at("text", default: ""), stream: if level == "warning" { "stderr" } else { "stdout" })
    }
  } else if item-type == "error" {
    _output-block(item.at("message", default: ""), stream: "stderr")
  } else if item-type == "display" or item-type == "result" {
    _render-display-item(item, label, opts, fig-labels)
  }
}

#let _render-results(label, opts) = {
  let results-path = sys.inputs.at("calepin-results", default: "")
  if results-path == "" {
    return none
  }
  let results-doc = json(results-path)
  let chunk = results-doc.at("chunks", default: (:)).at(label, default: none)
  if chunk == none {
    panic("calepin results do not contain label `" + label + "`")
  }
  let opts = _merge-result-options(opts, chunk)
  let fig-labels = _crossref-labels-for(chunk, "fig")
  let items = chunk.at("items", default: ())
  let image-group = ()
  for result-item in items {
    if _is-image-display-item(result-item, opts) {
      image-group.push(result-item)
    } else {
      if image-group.len() > 0 {
        if image-group.len() == 1 {
          _render-item(image-group.first(), label, opts, fig-labels)
        } else {
          _render-image-grid(image-group, label, opts, fig-labels)
        }
        image-group = ()
      }
      _render-item(result-item, label, opts, fig-labels)
    }
  }
  if image-group.len() > 0 {
    if image-group.len() == 1 {
      _render-item(image-group.first(), label, opts, fig-labels)
    } else {
      _render-image-grid(image-group, label, opts, fig-labels)
    }
  }
}
// Validate that document parameters only contain JSON-serializable leaves
// (none, bool, int, float, str) nested in arrays/dictionaries. Anything else
// (content, functions, lengths, colors, ...) fails fast with the offending path.
#let _validate-params(value, path) = {
  let t = type(value)
  if value == none or t == bool or t == int or t == float or t == str {
    // supported scalar leaf
  } else if t == array {
    for (i, item) in value.enumerate() {
      _validate-params(item, path + "[" + str(i) + "]")
    }
  } else if t == dictionary {
    for (k, v) in value.pairs() {
      _validate-params(v, if path == "" { k } else { path + "." + k })
    }
  } else {
    panic(
      "calepin.setup: unsupported parameter `" + path + "`: values of type " + str(t)
        + " cannot be passed as parameters; use none, a boolean, a number, a string, "
        + "an array, or a dictionary",
    )
  }
}

// Per-option defaults come from `_base-options` so there is a single source of
// truth for all document-level configuration.
#let setup(
  echo: _base-options.at("echo"),
  eval: _base-options.at("eval"),
  results: _base-options.at("results"),
  warning: _base-options.at("warning"),
  message: _base-options.at("message"),
  error: _base-options.at("error"),
  placeholder: _base-options.at("placeholder"),
  fig-device-format: _base-options.at("fig-device-format"),
  fig-device-dpi: _base-options.at("fig-device-dpi"),
  fig-device-width: _base-options.at("fig-device-width"),
  fig-device-height: _base-options.at("fig-device-height"),
  fig-device-aspect: _base-options.at("fig-device-aspect"),
  fig-width: _base-options.at("fig-width"),
  fig-height: _base-options.at("fig-height"),
  fig-align: _base-options.at("fig-align"),
  fig-responsive: _base-options.at("fig-responsive"),
  fenced-chunks: true,
  fallback-warning: true,
  theme: none,
  params: (:),
  ) = {
  _validate-params(params, "")
  let setup-opts = (
    echo: echo,
    eval: eval,
    results: results,
    warning: warning,
    message: message,
    error: error,
    placeholder: placeholder,
    "fig-device-format": fig-device-format,
    "fig-device-dpi": fig-device-dpi,
    "fig-device-width": fig-device-width,
    "fig-device-height": fig-device-height,
    "fig-device-aspect": fig-device-aspect,
    "fig-width": fig-width,
    "fig-height": fig-height,
    "fig-align": fig-align,
    "fig-responsive": fig-responsive,
    "fenced-chunks": fenced-chunks,
    "fallback-warning": fallback-warning,
    theme: theme,
    params: params,
  )
  _setup-defaults.update(defaults => (default: defaults.at("default") + setup-opts))
  if _mode == "query" {
    [#metadata(setup-opts) <calepin-config>]
  }
}

#let _coalesce-auto(value, fallback) = {
  if value == auto {
    fallback
  } else {
    value
  }
}

#let _resolve-options(engine, args) = {
  let defaults = _setup-defaults.get().at("default")
  let out = (:)
  for key in _base-options.keys() {
    out.insert(key, _coalesce-auto(args.at(key), defaults.at(key)))
  }
  for key in _call-extra-defaults.keys() {
    out.insert(key, args.at(key))
  }
  out
}

#let _chunk-spec(body, engine, label, crossref-labels, options) = {
  let out = (
    body: body,
    engine: engine,
    label: label,
    "crossref-labels": crossref-labels,
  )
  for key in _base-options.keys() {
    if key != "fenced-chunks" {
      out.insert(key, options.at(key))
    }
  }
  out
}

#let _query-crossref-placeholders(crossref-labels) = {
  let out = []
  for name in crossref-labels {
    if type(name) == str and name.starts-with("fig-") {
      out += [#figure(box(width: 0pt, height: 0pt), caption: none) #label(name)]
    }
  }
  out
}

#let _strip-qmd-label-quotes(value) = {
  let value = value.trim()
  if value.len() >= 2 and (
    (value.starts-with("\"") and value.ends-with("\"")) or
    (value.starts-with("'") and value.ends-with("'"))
  ) {
    value.slice(1, value.len() - 1)
  } else {
    value
  }
}

#let _parse-qmd-label-value(value) = {
  let value = value.trim()
  if value.starts-with("[") and value.ends-with("]") {
    let inner = value.slice(1, value.len() - 1).trim()
    if inner == "" {
      return ()
    }
    let labels = ()
    for item in inner.split(",") {
      labels.push(_strip-qmd-label-quotes(item))
    }
    labels
  } else {
    _strip-qmd-label-quotes(value)
  }
}

#let _qmd-label-from-body(body) = {
  let code = _raw-text(body)
  let code = if code.starts-with("\n") { code.slice(1) } else { code }
  for line in code.split("\n") {
    let trimmed = line.trim()
    if not trimmed.starts-with("#|") {
      return none
    }
    let directive = trimmed.slice(2).trim()
    let colon = directive.position(":")
    if colon == none {
      continue
    }
    let key = directive.slice(0, colon).trim()
    if key == "label" {
      return _parse-qmd-label-value(directive.slice(colon + 1))
    }
  }
  none
}

#let _label-name(value) = {
  let value = str(value)
  if value.starts-with("<") and value.ends-with(">") and value.len() >= 2 {
    value.slice(1, value.len() - 1)
  } else {
    value
  }
}

#let _metadata-fence-label(node) = {
  if node.has("label") and node.label == <calepin-fence-label> {
    let value = node.value
    if type(value) == dictionary and value.at("label", default: none) != none {
      return _label-name(value.at("label"))
    }
    panic("calepin.chunk: trailing fence label metadata is malformed")
  }
  none
}

#let _fence-label-from-body(body) = {
  let labels = ()
  let raw = _raw-node(body)
  if raw.has("label") {
    labels.push(_label-name(raw.label))
  }
  if body.has("children") {
    for child in body.children {
      let label = _metadata-fence-label(child)
      if label != none {
        labels.push(label)
      }
    }
  }
  if labels.len() > 1 {
    panic("calepin.chunk: label supplied more than once")
  }
  if labels.len() == 1 {
    labels.first()
  } else {
    none
  }
}

#let _strip-qmd-header(code) = {
  let out = ""
  let reading-header = true
  for line in code.split("\n") {
    if reading-header and line.trim().starts-with("#|") {
      continue
    }
    reading-header = false
    if out == "" {
      out = line
    } else {
      out += "\n" + line
    }
  }
  out
}

// Detect and strip a version suffix that Typst's fence parser split from
// the lang identifier.  For example, ```julia-1.2 produces lang="julia-1"
// with ".2\n" prepended to the code text.  This mirrors the
// reattach_version_suffix() logic in query.rs so the echo shows clean code.
#let _strip-lang-version-suffix(engine, code) = {
  let builtin-engines = ("python", "r", "mermaid", "dot", "tikz", "d2")
  if engine in builtin-engines { return code }
  let nl = code.position("\n")
  if nl == none { return code }
  let first-line = code.slice(0, nl)
  if not first-line.starts-with(".") or first-line.len() < 2 { return code }
  let tail = first-line.slice(1)
  let parts = tail.split(".")
  let is-version = parts.all(part =>
    part.len() > 0 and part.match(regex("^[0-9]+$")) != none
  )
  if not is-version { return code }
  code.slice(nl + 1)
}

#let _emit-chunk(engine, body, ..args) = context {
  let options = _call-defaults + args.named()
  let label-opt = options.at("label")
  let qmd-label-opt = _qmd-label-from-body(body)
  let fence-label-opt = _fence-label-from-body(body)
  let label-count = (
    if label-opt != none { 1 } else { 0 }
  ) + (
    if qmd-label-opt != none { 1 } else { 0 }
  ) + (
    if fence-label-opt != none { 1 } else { 0 }
  )
  if label-count > 1 {
    panic("calepin.chunk: label supplied more than once")
  }
  let label-opt = if qmd-label-opt != none {
    qmd-label-opt
  } else if fence-label-opt != none {
    fence-label-opt
  } else {
    label-opt
  }
  let auto-label-state = options.at("auto-label-state")
  let auto-label-prefix = options.at("auto-label-prefix")
  let derived = _derive-label(label-opt, auto-label-prefix, auto-label-state.get())
  let label = derived.id
  let crossref-labels = derived.names
  let generated-label = derived.generated
  let label-step = if generated-label {
    auto-label-state.update(n => n + 1)
  } else {
    _sync-auto-label-counter(auto-label-state, label)
  }
  if _mode == "query" {
    [
      #label-step
      #metadata(_chunk-spec(body, engine, label, crossref-labels, options)) <calepin-chunk>
      #_query-crossref-placeholders(crossref-labels)
    ]
  } else {
    let code = _raw-text(body)
    let code = if code.starts-with("\n") { code.slice(1) } else { code }
    let code = _strip-lang-version-suffix(engine, code)
    let code = _strip-qmd-header(code)
    let options = _resolve-options(engine, options)
    let show-echo = options.at("echo") == true
    let results-path = sys.inputs.at("calepin-results", default: "")
    label-step
    [#metadata((label: label, page: here().page())) <calepin-page>]

    if show-echo {
      _input-block(code, lang: engine)
    } else if results-path == "" {
      _input-block(code, lang: engine)
    }
    if results-path != "" {
      _render-results(label, options)
    }
  }
}

#let _without-raw-chunk-transforms(body) = context {
  let disabled = _disable-raw-chunk-transforms.get()
  _disable-raw-chunk-transforms.update(_ => true)
  let rendered = body()
  _disable-raw-chunk-transforms.update(_ => disabled)
  rendered
}

// `fenced-chunks` is the single switch for auto-running plain fenced blocks:
// `true` (every engine), an engine name, or a list of engine names.
#let _fenced-chunks-runs(engine, setting) = {
  if engine in ("typ", "typst") {
    false
  } else if setting == true {
    true
  } else if type(setting) == str {
    setting == engine
  } else if type(setting) == array {
    setting.contains(engine)
  } else {
    false
  }
}

#let chunk-from-raw-plain(engine, it) = {
  let defaults = _resolve-options(engine, _call-defaults)
  if _fenced-chunks-runs(engine, defaults.at("fenced-chunks")) {
    _emit-chunk(engine, it, ..defaults)
  } else {
    _html-themed-raw-block(it)
  }
}

#let _infer-engine(body) = {
  let node = _raw-node(body)
  if node.has("lang") and node.lang != none {
    node.lang
  } else {
    panic("calepin.chunk: no engine given; add a language to the fence (e.g. ```python) or pass the engine name")
  }
}

// `chunk` accepts either an explicit engine (`chunk("python")[...]`) or just a
// body (`chunk[```python ... ```]`), in which case the engine is read from the
// fenced block's language.
#let chunk(..args) = {
  let positional = args.pos()
  let engine = none
  let body = none
  if positional.len() >= 2 and type(positional.at(0)) == str {
    engine = positional.at(0)
    body = positional.at(1)
  } else if positional.len() >= 1 {
    body = positional.at(0)
    engine = _infer-engine(body)
  } else {
    panic("calepin.chunk: missing code block")
  }
  _without-raw-chunk-transforms(() => _emit-chunk(engine, body, ..args.named()))
}

#let inline(engine, body, ..args) = {
  let opts = args.named()
  if opts.at("label", default: none) != none {
    panic("unexpected argument: label")
  }
  let defaults = (
    echo: false,
    inline-output: true,
    auto-label-prefix: "inline",
    auto-label-state: _auto-inline-label-index,
  )
  chunk(engine, body, ..(defaults + opts))
}
