package org.l2jmobius.commons.database;

import java.lang.reflect.InvocationHandler;
import java.lang.reflect.Method;
import java.lang.reflect.Proxy;
import java.sql.Connection;
import java.sql.PreparedStatement;
import java.sql.ResultSet;
import java.sql.Statement;
import java.util.Set;
import java.util.TreeSet;

/**
 * Java-vectors harness replacement of {@code org.l2jmobius.commons.database.DatabaseFactory} (same public API).<br>
 * The vector generators must never open a database connection, not even a failing one. This class has no pool, no JDBC driver, no address and no credentials.
 * {@link #getConnection()} returns an in-memory fake ({@link Proxy}) that answers every query with zero rows and every update with zero changed rows, so the singletons that read
 * tables in their constructors ({@code CharInfoTable}, {@code SevenSigns}, ...) start with empty data, exactly like a fresh database. The SQL text of every statement is remembered
 * ({@link #statements()}) so a generator can report what the real code tried to read or write.
 */
public class DatabaseFactory
{
	private static final Set<String> STATEMENTS = new TreeSet<>();
	private static int _connections = 0;

	public static void init()
	{
	}

	public static synchronized Connection getConnection()
	{
		_connections++;
		return (Connection) Proxy.newProxyInstance(DatabaseFactory.class.getClassLoader(), new Class<?>[]
		{
			Connection.class
		}, new Fake("connection"));
	}

	public static void close()
	{
	}

	/** @return how many times the fake connection was requested */
	public static synchronized int connections()
	{
		return _connections;
	}

	/** @return the SQL text of every statement the code prepared or executed against the fake, sorted */
	public static synchronized Set<String> statements()
	{
		return new TreeSet<>(STATEMENTS);
	}

	private static synchronized void remember(String sql)
	{
		STATEMENTS.add(sql.trim().replaceAll("\\s+", " "));
	}

	private static Object proxy(Class<?> type, String kind)
	{
		return Proxy.newProxyInstance(DatabaseFactory.class.getClassLoader(), new Class<?>[]
		{
			type
		}, new Fake(kind));
	}

	private static final class Fake implements InvocationHandler
	{
		private final String _kind;

		Fake(String kind)
		{
			_kind = kind;
		}

		@Override
		public Object invoke(Object self, Method method, Object[] args)
		{
			final String name = method.getName();
			switch (name)
			{
				case "toString":
				{
					return "java-vectors fake " + _kind;
				}
				case "hashCode":
				{
					return System.identityHashCode(self);
				}
				case "equals":
				{
					return self == args[0];
				}
			}

			if ((args != null) && (args.length > 0) && (args[0] instanceof String) && (name.startsWith("prepare") || name.startsWith("execute") || name.equals("addBatch") || name.equals("nativeSQL")))
			{
				remember((String) args[0]);
			}

			switch (_kind)
			{
				case "connection":
				{
					switch (name)
					{
						case "prepareStatement", "prepareCall":
						{
							return proxy(PreparedStatement.class, "statement");
						}
						case "createStatement":
						{
							return proxy(Statement.class, "statement");
						}
						case "getAutoCommit":
						{
							return true;
						}
					}
					break;
				}
				case "statement":
				{
					switch (name)
					{
						case "executeQuery", "getResultSet", "getGeneratedKeys":
						{
							return proxy(ResultSet.class, "resultset");
						}
						case "executeBatch":
						{
							return new int[0];
						}
					}
					break;
				}
				case "resultset":
				{
					break;
				}
			}

			return defaultValue(method.getReturnType());
		}

		private static Object defaultValue(Class<?> type)
		{
			if (!type.isPrimitive() || (type == void.class))
			{
				return null;
			}
			if (type == boolean.class)
			{
				return false;
			}
			if (type == int.class)
			{
				return 0;
			}
			if (type == long.class)
			{
				return 0L;
			}
			if (type == double.class)
			{
				return 0.0;
			}
			if (type == float.class)
			{
				return 0.0f;
			}
			if (type == short.class)
			{
				return (short) 0;
			}
			if (type == byte.class)
			{
				return (byte) 0;
			}
			return (char) 0;
		}
	}
}
