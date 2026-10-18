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
 * <li>Supported values: {@code Map}, {@code List}, {@code String}, {@code Integer}, {@code Long}, {@code Boolean}, {@code byte[]} (written as a lowercase hex string) and {@code null}.</li>
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

	private static void value(StringBuilder sb, Object v, int level)
	{
		if (v == null)
		{
			sb.append("null");
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
