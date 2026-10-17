// Sign-in check for /account: a visitor without a session cookie is sent to /login.
export const onRequest = async (context, next) => {
	const session = context.cookies.get('session')?.value ?? null;
	context.locals.user = session;
	if (context.url.pathname === '/account' && !session) {
		return context.redirect('/login', 302);
	}
	return next();
};
