package l2rust.vectors;

import java.io.IOException;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;

/**
 * Tiny JSON writer for vector files (no dependency).
 * <ul>
 * <li>Objects are {@link LinkedHashMap}s: keys keep insertion order, so output is stable.</li>
 * <li>Supported values: {@code Map}, {@code List}, {@code String}, {@code Integer}, {@code Long}, {@code Double}, {@code Float}, {@code Boolean}, {@code byte[]} (written as a lowercase hex string) and {@code null}.</li>
 * <li>Output is pretty-printed with 2 spaces, LF line endings, UTF-8, one trailing newline. Same input gives byte-identical output.</li>
 * </ul>
 */
public final class Json
{
	private Json()
	{
	}

	/** Creates an empty ordered object. */
	public static Map<String, Object> obj()
	{
		return new LinkedHashMap<>();
	}

	/** Creates the standard object start every vector file has: {@code source} and {@code generated_by}. */
	public static Map<String, Object> header(String source)
	{
		final Map<String, Object> o = obj();
		o.put("source", source);
		o.put("generated_by", "java-vectors");
		return o;
	}

	/** Lowercase hex string of the bytes (empty array gives an empty string). */
	public static String hex(byte[] bytes)
	{
		final char[] digits = "0123456789abcdef".toCharArray();
		final char[] out = new char[bytes.length * 2];
		for (int i = 0; i < bytes.length; i++)
		{
			out[i * 2] = digits[(bytes[i] >> 4) & 0xF];
			out[(i * 2) + 1] = digits[bytes[i] & 0xF];
		}
		return new String(out);
	}

	/** Writes {@code root} to {@code file} (creating parent directories). */
	public static void write(Path file, Map<String, Object> root) throws IOException
	{
		final StringBuilder sb = new StringBuilder();
		value(sb, root, 0);
		sb.append('\n');
		Files.createDirectories(file.toAbsolutePath().getParent());
		Files.write(file, sb.toString().getBytes(StandardCharsets.UTF_8));
	}

	private static void indent(StringBuilder sb, int level)
	{
		for (int i = 0; i < level; i++)
		{
			sb.append("  ");
		}
	}

	/** Marks a value that is written on one line (no newlines, {@code ", "} and {@code ": "} separators) inside the pretty-printed file. */
	public static final class Compact
	{
		final Object value;

		Compact(Object value)
		{
			this.value = value;
		}
	}

	/** Wraps a map or list so it is written on a single line; keeps big vector files readable and small. */
	public static Compact compact(Object value)
	{
		return new Compact(value);
	}

	private static void inline(StringBuilder sb, Object v)
	{
		if (v instanceof Map<?, ?> m)
		{
			sb.append('{');
			int n = 0;
			for (Map.Entry<?, ?> e : m.entrySet())
			{
				string(sb, String.valueOf(e.getKey()));
				sb.append(": ");
				inline(sb, e.getValue());
				sb.append(++n < m.size() ? ", " : "");
			}
			sb.append('}');
		}
		else if (v instanceof List<?> l)
		{
			sb.append('[');
			for (int i = 0; i < l.size(); i++)
			{
				inline(sb, l.get(i));
				sb.append((i + 1) < l.size() ? ", " : "");
			}
			sb.append(']');
		}
		else
		{
			value(sb, v, 0);
		}
	}

	private static void value(StringBuilder sb, Object v, int level)
	{
		if (v == null)
		{
			sb.append("null");
		}
		else if (v instanceof Compact c)
		{
			inline(sb, c.value);
		}
		else if (v instanceof byte[] b)
		{
			string(sb, hex(b));
		}
		else if (v instanceof String s)
		{
			string(sb, s);
		}
		else if ((v instanceof Boolean) || (v instanceof Integer) || (v instanceof Long))
		{
			sb.append(v);
		}
		else if (v instanceof Double d)
		{
			// Double.toString is the shortest text that parses back to the same double ("1.0E10" is valid JSON). NaN and infinities have no JSON form: written as strings.
			if (Double.isNaN(d) || Double.isInfinite(d))
			{
				string(sb, d.toString());
			}
			else
			{
				sb.append(d);
			}
		}
		else if (v instanceof Float f)
		{
			// Float.toString is the shortest text that parses back to the same float; read it as f32 on the Rust side.
			if (Float.isNaN(f) || Float.isInfinite(f))
			{
				string(sb, f.toString());
			}
			else
			{
				sb.append(f);
			}
		}
		else if (v instanceof Map<?, ?> m)
		{
			if (m.isEmpty())
			{
				sb.append("{}");
				return;
			}
			sb.append("{\n");
			int n = 0;
			for (Map.Entry<?, ?> e : m.entrySet())
			{
				indent(sb, level + 1);
				string(sb, String.valueOf(e.getKey()));
				sb.append(": ");
				value(sb, e.getValue(), level + 1);
				sb.append(++n < m.size() ? ",\n" : "\n");
			}
			indent(sb, level);
			sb.append('}');
		}
		else if (v instanceof List<?> l)
		{
			if (l.isEmpty())
			{
				sb.append("[]");
				return;
			}
			sb.append("[\n");
			for (int i = 0; i < l.size(); i++)
			{
				indent(sb, level + 1);
				value(sb, l.get(i), level + 1);
				sb.append((i + 1) < l.size() ? ",\n" : "\n");
			}
			indent(sb, level);
			sb.append(']');
		}
		else
		{
			throw new IllegalArgumentException("unsupported JSON value: " + v.getClass());
		}
	}

	private static void string(StringBuilder sb, String s)
	{
		sb.append('"');
		for (int i = 0; i < s.length(); i++)
		{
			final char c = s.charAt(i);
			switch (c)
			{
				case '"' -> sb.append("\\\"");
				case '\\' -> sb.append("\\\\");
				case '\n' -> sb.append("\\n");
				case '\r' -> sb.append("\\r");
				case '\t' -> sb.append("\\t");
				default ->
				{
					if (c < 0x20)
					{
						sb.append(String.format("\\u%04x", (int) c));
					}
					else
					{
						sb.append(c);
					}
				}
			}
		}
		sb.append('"');
	}
}
